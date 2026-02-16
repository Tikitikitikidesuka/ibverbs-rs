#!/usr/bin/env python3
import argparse
import subprocess
import sys
import math
import numpy as np
from pathlib import Path
from typing import List

# ============================================================================
# Defaults
# ============================================================================
DEFAULT_ALGOS = ["dissemination", "binary-tree", "centralized"]
DEFAULT_SAMPLE_ITERS = 100
DEFAULT_BENCHMARK_ITERS = 1000
DEFAULT_STARTING_PORT = 10000

# Paths
DEFAULT_BASE_HOSTS = "../../devices/eb_devices.json"
CONFIG_GEN_SCRIPT = ["python3", "../../devices/device_config_gen.py"]
LAUNCHER_SCRIPT = ["python3", "mpi_barrier_benchmark.py"]
PLOT_SCRIPT = ["python3", "./plot_results.py"]


def generate_sizes(mode: str, start: int, end: int, num_samples: int) -> List[int]:
    """Generates a list of world sizes based on linear or log distribution."""
    if num_samples < 1:
        raise ValueError("samples must be >= 1")

    # Handle edge case where start == end
    if start == end:
        return [start]

    if mode == "linear":
        sizes = np.linspace(start, end, num_samples, dtype=int)
    elif mode == "log":
        # Calculate exponents for base 2
        start_exp = math.log2(start)
        end_exp = math.log2(end)
        sizes = np.logspace(start_exp, end_exp, num_samples, base=2, dtype=int)
    else:
        raise ValueError(f"Unknown mode: {mode}")

    # Deduplicate and sort
    return sorted(list(set(sizes)))


def validate_config(args):
    """
    Validates logical consistency of arguments to prevent nonsensical overlaps.
    """
    # 1. Check Generation Logic
    if args.dist:
        if args.min >= args.max:
            sys.exit(f"Error: --min ({args.min}) must be less than --max ({args.max}) when using distribution mode.")
        if args.samples < 1:
            sys.exit(f"Error: --samples must be >= 1.")

    # 2. Check for Ignore Warnings
    # If user provided manual sizes, but also set generation flags, warn them.
    if args.manual_sizes:
        # Check if defaults were changed by user (simple heuristic checks)
        if args.min != 16 or args.max != 128 or args.samples != 4:
            print(
                "⚠️  WARNING: You provided --manual-sizes. The arguments --min, --max, and --samples will be IGNORED.")


def main():
    parser = argparse.ArgumentParser(
        description="Run MPI Barrier Benchmark Sweep.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # 1. Log distribution (Default behavior if no sizes specified)
  python3 run_sweep.py --dist log --min 16 --max 128 --samples 4
  
  # 2. Linear distribution
  python3 run_sweep.py --dist linear --min 10 --max 100 --samples 5
  
  # 3. Explicit manual sizes (Overrides distribution settings)
  python3 run_sweep.py --manual-sizes 16 32 64 128
  
  # 4. Inject Jitter (500ns)
  python3 run_sweep.py --jitter-ns 500
        """
    )

    # --- Sweep Parameters ---
    parser.add_argument("--algos", nargs="+", default=DEFAULT_ALGOS,
                        help=f"Algorithms to benchmark (default: {' '.join(DEFAULT_ALGOS)})")

    # --- Size Definition (Mutually Exclusive) ---
    # User must pick EITHER Manual List OR Distribution Mode
    size_group = parser.add_mutually_exclusive_group()

    size_group.add_argument("--manual-sizes", nargs="+", type=int,
                            help="Space-separated integers (e.g., 16 32 64). Overrides generation params.")

    size_group.add_argument("--dist", choices=["linear", "log"], default="log",
                            help="Distribution mode for automatic generation (default: log)")

    # --- Generation Parameters (Grouped for clarity) ---
    gen_group = parser.add_argument_group("Generation Parameters (Used only with --dist)")
    gen_group.add_argument("--min", type=int, default=16, help="Start size (default: 16)")
    gen_group.add_argument("--max", type=int, default=128, help="End size (default: 128)")
    gen_group.add_argument("--samples", type=int, default=4, help="Number of samples (default: 4)")

    # --- Benchmark Config ---
    conf_group = parser.add_argument_group("Benchmark Configuration")
    conf_group.add_argument("--base-hosts", default=DEFAULT_BASE_HOSTS, help="Base JSON hosts file")
    conf_group.add_argument("--sample-iters", type=int, default=DEFAULT_SAMPLE_ITERS)
    conf_group.add_argument("--benchmark-iters", type=int, default=DEFAULT_BENCHMARK_ITERS)
    conf_group.add_argument("--starting-port", type=int, default=DEFAULT_STARTING_PORT)
    conf_group.add_argument("--allow-repetition", action="store_true", default=True,
                            help="Allow cycling hosts if world_size > physical nodes")
    conf_group.add_argument("--jitter-ns", type=int, default=0, help="Jitter injection in nanoseconds (default: 0)")
    conf_group.add_argument("--warmup-iters", type=int, default=1024, help="Number of iters for warmup (default: 1024)")

    # --- Output Control ---
    out_group = parser.add_argument_group("Output Control")
    out_group.add_argument("--output-dir", default="./sweep_outputs", help="Directory for all outputs")
    out_group.add_argument("--plot-title", default="Barrier Performance", help="Title for the generated plot")
    out_group.add_argument("--plot-output", default="barrier_comparison.png", help="Filename for plot output")

    # Note: Added x-scale arg here to match updated plot script requirement
    out_group.add_argument("--x-scale", choices=["linear", "log"], default="log", help="Plot X-axis scale (default: log)")

    args = parser.parse_args()

    # 1. Validate Logic
    validate_config(args)

    # 2. Determine Sizes
    sizes = []
    if args.manual_sizes:
        sizes = sorted(list(set(args.manual_sizes)))
        print(f"Mode: Manual | Sizes: {sizes}")
    else:
        # Default fallback is 'log' via argparse default
        try:
            sizes = generate_sizes(args.dist, args.min, args.max, args.samples)
            print(f"Mode: {args.dist.title()} Gen | Range: [{args.min}-{args.max}] | Samples: {args.samples}")
            print(f"Generated Sizes: {sizes}")
        except ValueError as e:
            sys.exit(f"Error generating sizes: {e}")

    if not sizes:
        sys.exit("Error: No world sizes determined.")

    # 3. Setup Directories
    out_dir = Path(args.output_dir)
    logs_dir = out_dir / "logs"
    configs_dir = out_dir / "configs"

    logs_dir.mkdir(parents=True, exist_ok=True)
    configs_dir.mkdir(parents=True, exist_ok=True)

    master_csv = out_dir / "master_results.csv"

    # Initialize CSV with header
    with open(master_csv, "w") as f:
        f.write("rank,world_size,algorithm,avg,std\n")

    # 4. Generate Configs
    print("\n=== Generating Configuration Files ===")
    for size in sizes:
        json_config = configs_dir / f"config_{size}.json"
        hostfile = configs_dir / f"hosts.{size}"
        rankfile = configs_dir / f"ranks.{size}"

        cmd = CONFIG_GEN_SCRIPT + [
            args.base_hosts,
            str(json_config),
            "--hostfile", str(hostfile),
            "--rankfile", str(rankfile),
            "--world-size", str(size),
            "--starting-port", str(args.starting_port)
        ]

        if args.allow_repetition:
            cmd.append("--allow-repetition")

        print(f"Generating for size {size}...")
        try:
            subprocess.run(cmd, check=True, capture_output=True)
        except subprocess.CalledProcessError as e:
            sys.exit(f"Error generating config for size {size}: {e}")

    # 5. Run Sweep
    print("\n=== Running Benchmark Sweep ===")
    total_runs = len(args.algos) * len(sizes)
    current_run = 0

    for algo in args.algos:
        for size in sizes:
            current_run += 1

            json_config = configs_dir / f"config_{size}.json"
            hostfile = configs_dir / f"hosts.{size}"
            rankfile = configs_dir / f"ranks.{size}"
            raw_log = logs_dir / f"log_{algo}_{size}.txt"

            print(f"[{current_run}/{total_runs}] Algorithm: {algo}, Size: {size}, Jitter: {args.jitter_ns}ns")

            # Construct command
            cmd = LAUNCHER_SCRIPT + [
                "--hostfile", str(hostfile),
                "--rankfile", str(rankfile),
                "--config-file", str(json_config),
                "--sample-iters", str(args.sample_iters),
                "--benchmark-iters", str(args.benchmark_iters),
                "--algorithm", algo,
                "--raw-log", str(raw_log),
                "--jitter-ns", str(args.jitter_ns),
                "--warmup-iters", str(args.warmup_iters)
            ]

            try:
                # Run and capture output
                result = subprocess.run(cmd, check=True, capture_output=True, text=True)

                # Append to master CSV (skipping header)
                lines = result.stdout.strip().split('\n')
                if len(lines) > 1:
                    data_lines = lines[1:]  # Skip header
                    with open(master_csv, "a") as f:
                        f.write('\n'.join(data_lines) + '\n')

            except subprocess.CalledProcessError as e:
                print(f"⚠️  Error running benchmark: {e}", file=sys.stderr)
                continue


if __name__ == "__main__":
    main()
