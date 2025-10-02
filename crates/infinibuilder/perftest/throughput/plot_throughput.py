#!/usr/bin/env python3
import pandas as pd
import matplotlib.pyplot as plt
import numpy as np
import argparse


def format_size(size_bytes):
    """Format size in bytes to KB/MB with powers of 2"""
    if size_bytes < 1024:
        return f"{size_bytes}B"
    elif size_bytes < 1024 * 1024:
        kb = size_bytes / 1024
        # Check if it's close to an integer (and thus likely a power of 2)
        if abs(kb - round(kb)) < 0.01:
            return f"{int(round(kb))}KB"
        return f"{kb:.0f}KB"
    else:
        mb = size_bytes / (1024 * 1024)
        # Check if it's close to an integer (and thus likely a power of 2)
        if abs(mb - round(mb)) < 0.01:
            return f"{int(round(mb))}MB"
        return f"{mb:.0f}MB"


def get_tick_positions(msg_sizes):
    """Get good tick positions for log2 scale"""
    min_log2 = np.log2(min(msg_sizes))
    max_log2 = np.log2(max(msg_sizes))

    # Generate all integer powers of 2 in range
    tick_powers = list(range(int(np.floor(min_log2)), int(np.ceil(max_log2)) + 1))

    # Select subset for readability
    power_range = tick_powers[-1] - tick_powers[0]
    if power_range > 20:
        # Show every other power
        tick_powers = tick_powers[::2]

    tick_positions = [2 ** p for p in tick_powers]
    tick_labels = [format_size(pos) for pos in tick_positions]

    return tick_positions, tick_labels


def plot_results(csv_file, output_prefix='benchmark', compare_csv=None):
    """Plot GBPS results from CSV, optionally comparing with another implementation"""
    # Rust orange and C++ blue colors
    rust_orange = '#CE422B'
    cpp_blue = '#00599C'

    # Read main data
    df = pd.read_csv(csv_file)

    msg_sizes = df['msg_size'].values
    mean_gbps = df['mean_gbps'].values
    std_gbps = df['std_gbps'].values

    # Read comparison data if provided
    compare_data = None
    if compare_csv:
        df_compare = pd.read_csv(compare_csv)
        # CSV format: s,mode,average,stddev
        compare_data = {
            'msg_sizes': df_compare['s'].values,
            'mean_gbps': df_compare['average'].values,
            'std_gbps': df_compare['stddev'].values
        }

    # Get tick positions based on main data
    tick_positions, tick_labels = get_tick_positions(msg_sizes)

    # Create GBPS plot
    fig, ax = plt.subplots(figsize=(10, 6))

    # Plot main data (Rust)
    ax.plot(msg_sizes, mean_gbps, color=rust_orange, linewidth=2, label='Rust')
    ax.fill_between(msg_sizes, mean_gbps - std_gbps, mean_gbps + std_gbps,
                    alpha=0.3, color=rust_orange)

    # Plot comparison data (C++) if provided
    if compare_data:
        ax.plot(compare_data['msg_sizes'], compare_data['mean_gbps'],
                color=cpp_blue, linewidth=2, label='C++')
        ax.fill_between(compare_data['msg_sizes'],
                        compare_data['mean_gbps'] - compare_data['std_gbps'],
                        compare_data['mean_gbps'] + compare_data['std_gbps'],
                        alpha=0.3, color=cpp_blue)
        ax.legend(loc='best', fontsize=11)

    ax.set_xscale('log', base=2)
    ax.set_xlabel('Message Size', fontsize=12)
    ax.set_ylabel('Throughput (Gbps)', fontsize=12)
    ax.grid(True, alpha=0.3)
    ax.set_xticks(tick_positions)
    ax.set_xticklabels(tick_labels, rotation=45, ha='right')

    plt.tight_layout()
    plt.savefig(f'{output_prefix}_gbps.svg', format='svg', bbox_inches='tight')
    print(f"Saved {output_prefix}_gbps.svg")
    plt.close()


def main():
    parser = argparse.ArgumentParser(description='Plot RDMA benchmark results')
    parser.add_argument('--input', type=str, required=True,
                        help='Input CSV file with benchmark results')
    parser.add_argument('--compare', type=str, default=None,
                        help='Optional CSV file to compare against (C++ implementation)')
    parser.add_argument('--output-prefix', type=str, default='benchmark',
                        help='Output filename prefix (default: benchmark)')

    args = parser.parse_args()

    plot_results(args.input, args.output_prefix, args.compare)


if __name__ == '__main__':
    main()