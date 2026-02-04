#!/usr/bin/env python3
import argparse
import subprocess
import sys
import re
import csv
import statistics
from collections import defaultdict
from typing import Dict, List, Optional, Tuple

# --- Regex Patterns ---
HEADER_RE = re.compile(r'barrier_benchmark\[(.*?)\]')
# Captures: [rank] -> timing ns
RANK_NS_RE = re.compile(r'\[(\d+)\]\s*->\s*(\d+)\s*ns')


def get_world_size_from_hostfile(hostfile_path: str) -> int:
    """Calculate world size by summing 'slots=N' from the hostfile."""
    total_slots = 0
    try:
        with open(hostfile_path, 'r') as f:
            for line in f:
                match = re.search(r'slots=(\d+)', line)
                if match:
                    total_slots += int(match.group(1))
                else:
                    if line.strip():
                        total_slots += 1
    except Exception as e:
        print(f"Error reading hostfile '{hostfile_path}': {e}", file=sys.stderr)
        sys.exit(1)

    if total_slots == 0:
        print(f"Error: No slots found in hostfile '{hostfile_path}'", file=sys.stderr)
        sys.exit(1)

    return total_slots


def parse_kv_list(s: str) -> Dict[str, str]:
    """Parses 'k=v, k2=v2' string into a dictionary."""
    out = {}
    for part in s.split(","):
        part = part.strip()
        if not part or "=" not in part:
            continue
        k, v = part.split("=", 1)
        out[k.strip()] = v.strip()
    return out


def main() -> None:
    p = argparse.ArgumentParser(description="Launch barrier benchmark and synthesize CSV results.")

    # --- Benchmark Execution Args ---
    p.add_argument("--hostfile", required=True, help="Path to MPI hostfile")
    p.add_argument("--rankfile", required=True, help="Path to MPI rankfile")
    p.add_argument("--config-file", required=True, help="Network config file for the benchmark")
    p.add_argument("--sample-iters", type=int, required=True, help="Number of sampling iterations")
    p.add_argument("--benchmark-iters", type=int, required=True, help="Number of benchmark iterations")
    p.add_argument("--algorithm", required=True, help="Barrier algorithm (e.g., dissemination)")
    p.add_argument("--jitter-ns", type=int, default=0, help="Jitter in nanoseconds (default: 0)")
    p.add_argument("--bin", default="./barrier_benchmark", help="Path to benchmark binary")

    # --- Output Control Args ---
    p.add_argument("--raw-log", type=str, default=None, help="Optional: Path to save the raw mpirun output text")

    args = p.parse_args()

    # 1. Auto-detect world size
    world_size = get_world_size_from_hostfile(args.hostfile)

    # 2. Construct command string
    rank_var = "${OMPI_COMM_WORLD_RANK:-${PMI_RANK}}"
    size_var = "${OMPI_COMM_WORLD_SIZE:-${PMI_SIZE}}"

    cmd_str = (
        f"{args.bin} "
        f"--config-file {args.config_file} "
        f"--world-size {size_var} "
        f"--sample-iters {args.sample_iters} "
        f"--algorithm {args.algorithm} "
        f"--rank {rank_var} "
        f"--benchmark-iters {args.benchmark_iters} "
        f"--jitter-ns {args.jitter_ns}"
    )

    # 3. Mpirun command
    mpirun_cmd = [
        "mpirun",
        "-n", str(world_size),
        "--bind-to", "numa",
        "--hostfile", args.hostfile,
        "--rankfile", args.rankfile,
        "bash", "-c", cmd_str
    ]

    # Open optional raw log file
    raw_log_f = open(args.raw_log, "w", encoding="utf-8") if args.raw_log else None

    # Data collection: Key = (rank, world_size, algorithm), Value = List[ns]
    samples: Dict[Tuple[int, int, str], List[int]] = defaultdict(list)

    # State tracking
    cur_algorithm = args.algorithm  # Default to arg, but update if seen in log
    cur_world_size = world_size  # Default to detected, update if seen in log

    # 4. Execute and Stream
    try:
        # Popen allows us to read stdout line-by-line while process runs
        with subprocess.Popen(
                mpirun_cmd,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,  # Merge stderr into stdout to capture all logs
                text=True,
                bufsize=1  # Line buffered
        ) as proc:

            # Read line by line
            if proc.stdout:
                for line in proc.stdout:
                    # Write to raw log if requested
                    if raw_log_f:
                        raw_log_f.write(line)

                    # Parse Header: barrier_benchmark[algorithm=...,world_size=...]
                    hm = HEADER_RE.search(line)
                    if hm:
                        kv = parse_kv_list(hm.group(1))
                        if "algorithm" in kv:
                            cur_algorithm = kv["algorithm"]
                        if "world_size" in kv:
                            try:
                                cur_world_size = int(kv["world_size"])
                            except ValueError:
                                pass

                    # Parse Data: [64] -> 78272 ns
                    tm = RANK_NS_RE.search(line)
                    if tm:
                        rank = int(tm.group(1))
                        ns = int(tm.group(2))
                        samples[(rank, cur_world_size, cur_algorithm)].append(ns)

            proc.wait()
            if proc.returncode != 0:
                print(f"Error: mpirun exited with code {proc.returncode}", file=sys.stderr)
                sys.exit(proc.returncode)

    except KeyboardInterrupt:
        sys.exit(130)
    finally:
        if raw_log_f:
            raw_log_f.close()

    # 5. Output CSV to Stdout
    writer = csv.writer(sys.stdout)
    writer.writerow(["rank", "world_size", "algorithm", "avg", "std"])

    # Sort by world_size, algorithm, then rank
    sorted_keys = sorted(samples.keys(), key=lambda k: (k[1], k[2], k[0]))

    for key in sorted_keys:
        rank, w_size, algo = key
        vals = samples[key]

        if not vals:
            continue

        avg = statistics.mean(vals)
        # Use population stdev (pstdev) or sample stdev (stdev).
        # Using pstdev here as it matches standard benchmark stats usually,
        # but switch to stdev if you prefer n-1.
        std = statistics.pstdev(vals) if len(vals) > 1 else 0.0

        writer.writerow([rank, w_size, algo, f"{avg:.2f}", f"{std:.2f}"])


if __name__ == "__main__":
    main()
