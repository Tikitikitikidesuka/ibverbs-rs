#!/usr/bin/env python3
import json
import subprocess
import sys
import threading
import re
import statistics
from pathlib import Path
import argparse

# ---------------------------
# Argument parsing
# ---------------------------
parser = argparse.ArgumentParser(description="Run barrier_perftest on multiple nodes via SSH.")
parser.add_argument("--config", required=True, type=Path,
                    help="Name of the file with the JSON config")
parser.add_argument("--binary", required=True, type=Path,
                    help="Path to barrier_perftest executable on remote hosts")
parser.add_argument("--num-nodes", required=True, type=int, help="Number of nodes to run")
parser.add_argument("--output", required=True, type=Path, help="Output file for summary")
parser.add_argument("--iters", type=int, default=10, help="Number of iterations per barrier test")
parser.add_argument("--batch-size", type=int, default=1024, help="Batch size for barrier_perftest")
parser.add_argument("--algorithm", type=str, default="centralized", help="Barrier algorithm to use")
args = parser.parse_args()

# ---------------------------
# Load local host config
# ---------------------------
if not args.config.is_file():
    print(f"Local config file '{args.config}' does not exist.")
    sys.exit(1)

with open(args.config) as f:
    config = json.load(f)

hosts = config["hosts"][:args.num_nodes]
if args.num_nodes > len(config["hosts"]):
    print(f"Requested {args.num_nodes} nodes, but only {len(config['hosts'])} in config")
    sys.exit(1)

# ---------------------------
# Prepare data structures
# ---------------------------
node_times = {h["hostname"]: [] for h in hosts}
lock = threading.Lock()
barrier_pattern = re.compile(r"\[(\d+)\] -> Barrier in (\d+) ns")

# ---------------------------
# Copy config to remote host and run barrier_perftest
# ---------------------------
def run_node(host_info):
    host = host_info["hostname"] + ".lbdaq.cern.ch"
    rank_id = host_info["rankid"]

    remote_config_path = f"{args.binary.parent}/{args.config.name}"

    ssh_cmd = [
        "ssh", host,
        f"{args.binary} --config-file {remote_config_path} "
        f"--batch-size {args.batch_size} --algorithm {args.algorithm} "
        f"--iters {args.iters} --num-nodes {args.num_nodes} --rank-id {rank_id}"
    ]

    process = subprocess.Popen(ssh_cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, bufsize=1)

    for line in process.stdout:
        print(f"[{host_info['hostname']}] {line}", end="")
        match = barrier_pattern.search(line)
        if match:
            time_ns = int(match.group(2))
            with lock:
                node_times[host_info["hostname"]].append(time_ns)

    process.wait()
    if process.returncode != 0:
        print(f"[{host_info['hostname']}] WARNING: process exited with code {process.returncode}")

# ---------------------------
# Launch threads
# ---------------------------
threads = []
for h in hosts:
    t = threading.Thread(target=run_node, args=(h,))
    t.start()
    threads.append(t)

for t in threads:
    t.join()

# ---------------------------
# Compute statistics
# ---------------------------
def compute_stats(times):
    avg = statistics.mean(times) if times else 0
    std = statistics.stdev(times) if len(times) > 1 else 0
    return avg, std

all_times = []
lines = ["Barrier Performance Summary", "=========================="]
for host, times in node_times.items():
    avg, std = compute_stats(times)
    lines.append(f"Host: {host}")
    lines.append(f"  Avg: {avg:.2f} ns")
    lines.append(f"  Std: {std:.2f} ns")
    lines.append("")
    all_times.extend(times)

overall_avg, overall_std = compute_stats(all_times)
lines.append("Overall")
lines.append(f"  Avg: {overall_avg:.2f} ns")
lines.append(f"  Std: {overall_std:.2f} ns")

# ---------------------------
# Write summary to output file
# ---------------------------
with open(args.output, "w") as f:
    f.write("\n".join(lines))

print(f"Results saved to {args.output}")