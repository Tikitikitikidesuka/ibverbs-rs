#!/usr/bin/env python3
import argparse
import subprocess
import sys
import re
import csv
import statistics
from collections import defaultdict
from typing import Dict, List, Tuple

# --- Regex Patterns ---
HEADER_RE = re.compile(r'barrier_benchmark\[(.*?)\]')
# Matches lowercase "mean:" and "std:"
# Log line: "[0] -> mean: 13930.64 ns, std: 6940.22 ns"
RANK_STATS_RE = re.compile(r'\[(\d+)\]\s*->\s*mean:\s*([\d\.]+)\s*ns,\s*std:\s*([\d\.]+)\s*ns', re.IGNORECASE)


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
    p.add_argument("--sample-iters", type=int, required=True, help="Number of sampling iterations inside Rust")
    p.add_argument("--benchmark-iters", type=int, required=True, help="Number of times to run the benchmark loop")
    p.add_argument("--warmup-iters", type=int, default=128, help="Number of warmup iterations (default: 128)")
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
        f"--warmup-iters {args.warmup_iters} "
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

    # Data collection: Key = (rank, world_size, algorithm), Value = List[(mean, std)]
    # We store tuples of (mean, std) because Rust now reports both.
    samples: Dict[Tuple[int, int, str], List[Tuple[float, float]]] = defaultdict(list)

    # State tracking
    cur_algorithm = args.algorithm
    cur_world_size = world_size

    # 4. Execute and Stream
    try:
        with subprocess.Popen(
                mpirun_cmd,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,  # Merge stderr into stdout
                text=True,
                bufsize=1  # Line buffered
        ) as proc:

            if proc.stdout:
                for line in proc.stdout:
                    if raw_log_f:
                        raw_log_f.write(line)

                    # Parse Header
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

                    # Parse Data: [64] -> Mean: 78272.50 ns, Std: 120.00 ns
                    tm = RANK_STATS_RE.search(line)
                    if tm:
                        rank = int(tm.group(1))
                        mean_val = float(tm.group(2))
                        std_val = float(tm.group(3))
                        samples[(rank, cur_world_size, cur_algorithm)].append((mean_val, std_val))

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
        data_points = samples[key]

        if not data_points:
            continue

        # Extract lists
        means = [d[0] for d in data_points]
        stds = [d[1] for d in data_points]

        # Calculate aggregations
        # If benchmark_iters > 1, we average the Means and average the Stds reported by Rust.
        final_avg = statistics.mean(means)
        final_std = statistics.mean(stds)

        writer.writerow([rank, w_size, algo, f"{final_avg:.2f}", f"{final_std:.2f}"])


if __name__ == "__main__":
    main()
