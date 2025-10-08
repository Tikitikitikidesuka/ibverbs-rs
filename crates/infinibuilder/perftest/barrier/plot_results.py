#!/usr/bin/env python3
import argparse
import pandas as pd
import matplotlib.pyplot as plt
from pathlib import Path

parser = argparse.ArgumentParser(description="Plot barrier test results.")
parser.add_argument("--input", type=Path, required=True, help="Input CSV file")
parser.add_argument("--output", type=Path, default=None, help="Output plot file (default: input.png)")
parser.add_argument("--title", type=str, default="Barrier Performance vs Node Count", help="Plot title")
parser.add_argument("--error-bars", action="store_true", help="Show error bars (std dev)")
parser.add_argument("--show-range", action="store_true", help="Show min-max range as shaded area")
args = parser.parse_args()

if not args.input.is_file():
    print(f"Error: Input file '{args.input}' does not exist")
    exit(1)

# Read data
df = pd.read_csv(args.input)

# Validate required columns
if 'nodes' not in df.columns or 'avg_ns' not in df.columns:
    print("Error: CSV must contain 'nodes' and 'avg_ns' columns")
    exit(1)

# Create figure
fig, ax = plt.subplots(figsize=(10, 6))

# Plot average with optional error bars
if args.error_bars and 'std_ns' in df.columns:
    ax.errorbar(df['nodes'], df['avg_ns'], yerr=df['std_ns'],
                marker='o', linestyle='-', capsize=5, label='Average ± Std Dev')
else:
    ax.plot(df['nodes'], df['avg_ns'], marker='o', linestyle='-', label='Average', linewidth=2)

# Optional: plot min/max range
if args.show_range and 'min_ns' in df.columns and 'max_ns' in df.columns:
    ax.fill_between(df['nodes'], df['min_ns'], df['max_ns'], alpha=0.2, label='Min-Max Range')

ax.set_xlabel('Number of Nodes', fontsize=12)
ax.set_ylabel('Barrier Time (ns)', fontsize=12)
ax.set_title(args.title, fontsize=14)
ax.grid(True, alpha=0.3)
ax.legend()

# Set output file
if args.output is None:
    args.output = args.input.with_suffix('.png')

plt.tight_layout()
plt.savefig(args.output, dpi=300)
print(f"Plot saved to {args.output}")
