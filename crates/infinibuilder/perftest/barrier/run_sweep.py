#!/usr/bin/env python3
import subprocess
import argparse
import json
from pathlib import Path
import re
import statistics
import sys

parser = argparse.ArgumentParser(description="Run barrier tests across a range of node counts and generate plots.")
parser.add_argument("--start", type=int, default=2, help="Starting number of nodes (default: 2)")
parser.add_argument("--end", type=int, required=True, help="Ending number of nodes")
parser.add_argument("--step", type=int, required=True, help="Step size")
parser.add_argument("--output-dir", type=Path, required=True, help="Output directory")
parser.add_argument("--run-script", type=Path, required=True,
                    help="Path to run script")
parser.add_argument("--parse-script", type=Path, default=None,
                    help="Path to parse_barrier_results.py (optional, uses built-in parsing if not provided)")
parser.add_argument("--binary", type=Path, help="Path to barrier_perftest binary")
parser.add_argument("--algorithm", type=str, required=True, help="Algorithm to use")
parser.add_argument("--iters", type=int, default=10, help="Iterations per test (default: 10)")
parser.add_argument("--batch-size", type=int, default=1024, help="Batch size (default: 1024)")
parser.add_argument("--remove-outliers", action="store_true",
                    help="Remove outliers when parsing results (requires --parse-script)")
args = parser.parse_args()

# Resolve and check run script path
run_script = args.run_script.resolve()
if not run_script.is_file():
    print(f"Error: Run script '{args.run_script}' does not exist")
    sys.exit(1)

# Check parse script if provided
if args.parse_script:
    parse_script = args.parse_script.resolve()
    if not parse_script.is_file():
        print(f"Error: Parse script '{args.parse_script}' does not exist")
        sys.exit(1)
elif args.remove_outliers:
    print("Error: --remove-outliers requires --parse-script")
    sys.exit(1)

# Create output directory
args.output_dir.mkdir(exist_ok=True)

# Store results
results = []

print(f"Running tests from {args.start} to {args.end} nodes (step={args.step})")
print(f"Output directory: {args.output_dir}")
if args.remove_outliers:
    print("Outlier removal: ENABLED")
print("="*60)

# Run tests for each node count
for num_nodes in range(args.start, args.end + 1, args.step):
    output_file = args.output_dir / f"raw_{num_nodes}nodes.txt"

    print(f"\nTesting {num_nodes} nodes...")

    # Build command
    cmd = [
        str(run_script),
        "--num-nodes", str(num_nodes),
        "--output", str(output_file.resolve()),
        "--algorithm", args.algorithm,
        "--iters", str(args.iters),
        "--batch-size", str(args.batch_size)
    ]

    if args.binary:
        cmd.extend(["--binary", str(args.binary)])

    # Run the test
    try:
        result = subprocess.run(cmd, check=True, capture_output=True, text=True)
        print(f"  ✓ Completed")
    except subprocess.CalledProcessError as e:
        print(f"  ✗ Failed: {e}")
        print(f"  stdout: {e.stdout}")
        print(f"  stderr: {e.stderr}")
        continue
    except FileNotFoundError as e:
        print(f"  ✗ File not found: {e}")
        sys.exit(1)

    if not output_file.exists():
        print(f"  ✗ Output file not created")
        continue

    # Parse the output
    if args.parse_script:
        # Use external parse script
        summary_file = args.output_dir / f"summary_{num_nodes}nodes.txt"
        parse_cmd = [
            "python3", str(parse_script),
            "--input", str(output_file),
            "--output", str(summary_file)
        ]

        if args.remove_outliers:
            parse_cmd.append("--remove-outliers")

        try:
            parse_result = subprocess.run(parse_cmd, check=True, capture_output=True, text=True)

            # Extract stats from parse output
            avg_match = re.search(r"Avg: ([0-9.]+) ns", parse_result.stdout)
            std_match = re.search(r"Std: ([0-9.]+) ns", parse_result.stdout)
            outliers_match = re.search(r"removed (\d+) outlier", parse_result.stdout)

            if avg_match and std_match:
                avg = float(avg_match.group(1))
                std = float(std_match.group(1))

                results.append({
                    "nodes": num_nodes,
                    "avg_ns": avg,
                    "std_ns": std
                })

                output_msg = f"  Avg: {avg:.2f} ns, Std: {std:.2f} ns"
                if outliers_match:
                    outlier_count = int(outliers_match.group(1))
                    output_msg += f" (removed {outlier_count} outliers)"
                print(output_msg)
            else:
                print(f"  ✗ Could not parse statistics from output")

        except subprocess.CalledProcessError as e:
            print(f"  ✗ Parse failed: {e}")
            continue
    else:
        # Use built-in parsing (no outlier removal)
        barrier_pattern = re.compile(r"\[(\d+)\] -> Barrier in (\d+) ns")
        times = []

        with open(output_file) as f:
            for line in f:
                match = barrier_pattern.search(line)
                if match:
                    times.append(int(match.group(2)))

        if times:
            avg = statistics.mean(times)
            std = statistics.stdev(times) if len(times) > 1 else 0
            min_time = min(times)
            max_time = max(times)

            results.append({
                "nodes": num_nodes,
                "avg_ns": avg,
                "std_ns": std,
                "min_ns": min_time,
                "max_ns": max_time,
                "count": len(times)
            })

            print(f"  Avg: {avg:.2f} ns, Std: {std:.2f} ns")
        else:
            print(f"  ✗ No barrier times found in output")

# Save summary
summary_file = args.output_dir / "summary.json"
with open(summary_file, "w") as f:
    json.dump(results, f, indent=2)

print(f"\n{'='*60}")
print(f"Summary saved to {summary_file}")
print(f"Total tests completed: {len(results)}")

# Create CSV for plotting
csv_file = args.output_dir / "results.csv"
with open(csv_file, "w") as f:
    if results and "min_ns" in results[0]:
        f.write("nodes,avg_ns,std_ns,min_ns,max_ns\n")
        for r in results:
            f.write(f"{r['nodes']},{r['avg_ns']},{r['std_ns']},{r['min_ns']},{r['max_ns']}\n")
    else:
        f.write("nodes,avg_ns,std_ns\n")
        for r in results:
            f.write(f"{r['nodes']},{r['avg_ns']},{r['std_ns']}\n")

print(f"CSV saved to {csv_file}")
print(f"\nTo plot results, use:")
print(f"  python3 plot_results.py --input {csv_file}")
