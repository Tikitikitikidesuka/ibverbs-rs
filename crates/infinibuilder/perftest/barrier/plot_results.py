#!/usr/bin/env python3
import argparse
import pandas as pd
import matplotlib.pyplot as plt
from pathlib import Path

parser = argparse.ArgumentParser(description="Plot barrier test results.")
parser.add_argument("--input", type=Path, nargs='+', required=True,
                    help="Input CSV file(s) - can specify multiple files")
parser.add_argument("--labels", type=str, nargs='+', default=None,
                    help="Labels for each input file (optional, uses filenames if not provided)")
parser.add_argument("--output", type=str, default=None,
                    help="Output filename without extension (default: barrier_results)")
parser.add_argument("--title", type=str, default="Barrier Performance vs Node Count", help="Plot title")
parser.add_argument("--error-bars", action="store_true", help="Show error bars (std dev)")
parser.add_argument("--std-band", action="store_true", help="Show std dev as shaded band around line")
parser.add_argument("--show-range", action="store_true", help="Show min-max range as shaded area")
args = parser.parse_args()

# Validate inputs
for input_file in args.input:
    if not input_file.is_file():
        print(f"Error: Input file '{input_file}' does not exist")
        exit(1)

# Check labels
if args.labels:
    if len(args.labels) != len(args.input):
        print(f"Error: Number of labels ({len(args.labels)}) must match number of input files ({len(args.input)})")
        exit(1)
else:
    # Use filenames as labels
    args.labels = [f.stem for f in args.input]

# Set output filename
if args.output is None:
    args.output = "barrier_results"
elif args.output.endswith('.png') or args.output.endswith('.svg'):
    # Remove extension if provided
    args.output = Path(args.output).stem

# Color palette - consistent colors for multiple datasets
colors = ['#1f77b4', '#ff7f0e', '#2ca02c', '#d62728', '#9467bd',
          '#8c564b', '#e377c2', '#7f7f7f', '#bcbd22', '#17becf']

# Create figure
fig, ax = plt.subplots(figsize=(10, 6))

# Plot each dataset
for idx, (input_file, label) in enumerate(zip(args.input, args.labels)):
    # Read data
    df = pd.read_csv(input_file)

    # Validate required columns
    if 'nodes' not in df.columns or 'avg_ns' not in df.columns:
        print(f"Warning: '{input_file}' missing required columns, skipping")
        continue

    color = colors[idx % len(colors)]

    # Plot average line
    ax.plot(df['nodes'], df['avg_ns'], marker='o', linestyle='-',
            label=label, linewidth=2, color=color)

    # Optional: error bars
    if args.error_bars and 'std_ns' in df.columns:
        ax.errorbar(df['nodes'], df['avg_ns'], yerr=df['std_ns'],
                    fmt='none', capsize=5, color=color, alpha=0.5)

    # Optional: std dev band
    if args.std_band and 'std_ns' in df.columns:
        ax.fill_between(df['nodes'],
                        df['avg_ns'] - df['std_ns'],
                        df['avg_ns'] + df['std_ns'],
                        alpha=0.2, color=color)

    # Optional: min/max range
    if args.show_range and 'min_ns' in df.columns and 'max_ns' in df.columns:
        ax.fill_between(df['nodes'], df['min_ns'], df['max_ns'],
                        alpha=0.1, color=color)

ax.set_xlabel('Number of Nodes', fontsize=12)
ax.set_ylabel('Barrier Time (ns)', fontsize=12)
ax.set_title(args.title, fontsize=14)
ax.grid(True, alpha=0.3)
ax.legend()

plt.tight_layout()

# Save as both PNG and SVG
png_file = f"{args.output}.png"
svg_file = f"{args.output}.svg"

plt.savefig(png_file, dpi=300, format='png')
plt.savefig(svg_file, format='svg')

print(f"Plots saved:")
print(f"  PNG: {png_file}")
print(f"  SVG: {svg_file}")
