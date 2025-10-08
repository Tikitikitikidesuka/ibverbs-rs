#!/usr/bin/env python3
import subprocess
import sys

# Configuration
num_nodes = 9  # Change this
algorithm = "centralized"  # Change this

# Fixed parameters
config = "config.json"
binary = "/home/mhermoso/Documents/pcie40-rs/target/release/barrier_perftest"
batch_size = 1024
iters = 10

# Generate output name
output = f"{num_nodes}_{algorithm}"

# Build and run command
cmd = [
    "python", "run_barrier.py",
    "--config", config,
    "--binary", binary,
    "--num-nodes", str(num_nodes),
    "--batch-size", str(batch_size),
    "--iters", str(iters),
    "--output", output,
    "--algorithm", algorithm
]

print(f"Running: {' '.join(cmd)}")
subprocess.run(cmd)
