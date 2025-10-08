#!/usr/bin/env python3
import re
import statistics
from pathlib import Path
import argparse

def remove_outliers_iqr(data):
    """Remove outliers using the IQR method."""
    if len(data) < 4:
        return data

    sorted_data = sorted(data)
    n = len(sorted_data)
    q1 = sorted_data[n // 4]
    q3 = sorted_data[(3 * n) // 4]
    iqr = q3 - q1

    lower_bound = q1 - 1.5 * iqr
    upper_bound = q3 + 1.5 * iqr

    filtered = [x for x in data if lower_bound <= x <= upper_bound]
    return filtered if filtered else data

parser = argparse.ArgumentParser(description="Parse barrier_perftest output from log file.")
parser.add_argument("--input", required=True, type=Path,
                    help="Input log file containing barrier_perftest output")
parser.add_argument("--output", required=True, type=Path,
                    help="Output file for summary")
parser.add_argument("--per-rank", action="store_true",
                    help="Show statistics per rank (default: only overall)")
parser.add_argument("--remove-outliers", action="store_true",
                    help="Remove outliers using IQR method")
args = parser.parse_args()

if not args.input.is_file():
    print(f"Input file '{args.input}' does not exist.")
    exit(1)

with open(args.input) as f:
    lines = f.readlines()

rank_times = {}
barrier_pattern = re.compile(r"\[(\d+)\] -> Barrier in (\d+) ns")

for line in lines:
    barrier_match = barrier_pattern.search(line)
    if barrier_match:
        rank_id = int(barrier_match.group(1))
        time_ns = int(barrier_match.group(2))

        if rank_id not in rank_times:
            rank_times[rank_id] = []
        rank_times[rank_id].append(time_ns)

if not rank_times:
    print("Warning: No barrier times found in input file")
    exit(1)

def compute_stats(times):
    avg = statistics.mean(times) if times else 0
    std = statistics.stdev(times) if len(times) > 1 else 0
    return avg, std

all_times = []
output_lines = []

# Show per-rank stats only if flag is set
if args.per_rank:
    for rank_id in sorted(rank_times.keys()):
        times = rank_times[rank_id]
        original_count = len(times)

        # Remove outliers if requested
        if args.remove_outliers:
            times = remove_outliers_iqr(times)

        avg, std = compute_stats(times)
        output_lines.append(f"Rank {rank_id}:")
        output_lines.append(f"  Avg: {avg:.2f} ns")
        output_lines.append(f"  Std: {std:.2f} ns")
        if args.remove_outliers and len(times) < original_count:
            output_lines.append(f"  (removed {original_count - len(times)} outliers)")
        output_lines.append("")
        all_times.extend(times)
else:
    # Just collect all times for overall stats
    for times in rank_times.values():
        all_times.extend(times)

# Remove outliers from overall stats if requested
original_total = len(all_times)
if args.remove_outliers:
    all_times = remove_outliers_iqr(all_times)

overall_avg, overall_std = compute_stats(all_times)
output_lines.append("Overall")
output_lines.append(f"  Avg: {overall_avg:.2f} ns")
output_lines.append(f"  Std: {overall_std:.2f} ns")
if args.remove_outliers and len(all_times) < original_total:
    output_lines.append(f"  (removed {original_total - len(all_times)} outliers)")

with open(args.output, "w") as f:
    f.write("\n".join(output_lines) + "\n")

print(f"Results saved to {args.output}")
print("\n".join(output_lines))
