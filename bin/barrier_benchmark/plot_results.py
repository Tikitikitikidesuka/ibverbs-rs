#!/usr/bin/env python3
import argparse
import pandas as pd
import matplotlib.pyplot as plt
from pathlib import Path
import sys


def main():
    parser = argparse.ArgumentParser(description="Plot barrier test results from master CSV.")

    # Input
    parser.add_argument("input", type=Path, help="Input master CSV file")

    # Output (Mandatory, with extension)
    parser.add_argument("--output", type=str, required=True,
                        help="Output filename with extension (e.g., 'results/plot.png'). Format inferred from extension.")

    # Plotting Options
    parser.add_argument("--title", type=str, default="Barrier Performance vs Node Count", help="Plot title")

    # Axis Scale Option (REQUIRED - No Default)
    parser.add_argument("--x-scale", choices=["linear", "log"], required=True,
                        help="X-axis scale type (linear or log)")

    # Toggles (Default: OFF)
    parser.add_argument("--std-bands", action="store_true", help="Show std dev as shaded band around line")
    parser.add_argument("--std-bars", action="store_true", help="Show error bars (std dev)")

    # New Toggle: Show Markers (Default: OFF)
    parser.add_argument("--markers", action="store_true", help="Show dots/markers on the lines (default: hidden)")

    args = parser.parse_args()

    # 1. Validation
    if not args.input.is_file():
        sys.exit(f"Error: Input file '{args.input}' does not exist")

    # 2. Load Data
    try:
        df = pd.read_csv(args.input)
    except Exception as e:
        sys.exit(f"Error reading CSV: {e}")

    # Ensure numeric types
    for col in ['avg', 'std', 'world_size']:
        df[col] = pd.to_numeric(df[col], errors='coerce')
    df.dropna(inplace=True)

    # 3. Aggregate Data
    grouped = df.groupby(['algorithm', 'world_size']).agg({
        'avg': 'mean',
        'std': 'mean'
    }).reset_index()

    # 4. Setup Plot
    colors = ['#1f77b4', '#ff7f0e', '#2ca02c', '#d62728', '#9467bd',
              '#8c564b', '#e377c2', '#7f7f7f', '#bcbd22', '#17becf']

    fig, ax = plt.subplots(figsize=(10, 6))

    algorithms = sorted(grouped['algorithm'].unique())

    # 5. Plot Loop
    for idx, algo in enumerate(algorithms):
        subset = grouped[grouped['algorithm'] == algo].sort_values("world_size")

        color = colors[idx % len(colors)]
        label = algo

        # Determine marker style based on argument (Default is None)
        marker_style = 'o' if args.markers else None

        # A. Plot Main Line (Markersize is 4 if enabled)
        ax.plot(subset['world_size'], subset['avg'], marker=marker_style, markersize=4, linestyle='-',
                label=label, linewidth=2, color=color)

        # B. Optional: Error Bars
        if args.std_bars:
            ax.errorbar(subset['world_size'], subset['avg'], yerr=subset['std'],
                        fmt='none', capsize=5, color=color, alpha=0.5)

        # C. Optional: Std Dev Band
        if args.std_bands:
            ax.fill_between(subset['world_size'],
                            subset['avg'] - subset['std'],
                            subset['avg'] + subset['std'],
                            alpha=0.2, color=color)

    # 6. Formatting
    ax.set_xlabel('Number of Nodes', fontsize=12)
    ax.set_ylabel('Barrier Time (ns)', fontsize=12)
    ax.set_title(args.title, fontsize=14)
    ax.grid(True, alpha=0.3)
    ax.legend()

    # Apply X-Axis Scale
    if args.x_scale == "log":
        ax.set_xscale('log', base=2)
    elif args.x_scale == "linear":
        ax.set_xscale('linear')

    plt.tight_layout()

    # 7. Save Output
    out_path = Path(args.output)

    # Ensure parent directory exists
    if out_path.parent != Path('.'):
        out_path.parent.mkdir(parents=True, exist_ok=True)

    # Save using the provided filename and extension
    try:
        plt.savefig(out_path, dpi=300)
        print(f"Plot saved to: {out_path}")
    except Exception as e:
        sys.exit(f"Error saving plot: {e}")


if __name__ == "__main__":
    main()
