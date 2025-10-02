#!/usr/bin/env python3
import subprocess
import re
import argparse
import csv
import numpy as np


def parse_output_line(line):
    """Parse a line like 'Pps: 359.12, Gbps: 192.78' and return (pps, gbps)"""
    match = re.search(r'[Pp]ps:\s*([\d.]+),\s*[Gg]bps:\s*([\d.]+)', line, re.IGNORECASE)
    if match:
        return float(match.group(1)), float(match.group(2))
    return None


def run_benchmark(binary_path, config_file, role, message_size, batch_size, iters):
    """Run the benchmark and collect pps/gbps samples"""
    cmd = [
        str(binary_path),
        '--config-file', config_file,
        '--role', role,
        '--num-nodes', '2',
        '--message-size', str(message_size),
        '--batch-size', str(batch_size),
        '--iters', str(iters)
    ]

    try:
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=300)

        pps_samples = []
        gbps_samples = []

        output = result.stdout + result.stderr
        for line in output.split('\n'):
            parsed = parse_output_line(line)
            if parsed:
                pps, gbps = parsed
                pps_samples.append(pps)
                gbps_samples.append(gbps)

        if pps_samples:
            print(f"  Collected {len(pps_samples)} samples")
        else:
            print(f"  Warning: No samples collected")

        return pps_samples, gbps_samples

    except subprocess.TimeoutExpired:
        print(f"  Warning: Benchmark timed out after 300s")
        return [], []
    except Exception as e:
        print(f"  Error running benchmark: {e}")
        return [], []


def collect_samples(binary_path, config_file, rank_id, message_size, batch_size,
                   max_samples, mean_window_size, std_threshold=0.05):
    """Collect samples and check for convergence using a sliding window"""
    pps_samples, gbps_samples = run_benchmark(
        binary_path, config_file, rank_id, message_size, batch_size, max_samples
    )

    if not pps_samples or len(pps_samples) < mean_window_size:
        return pps_samples, gbps_samples

    # Try sliding windows of size mean_window_size
    for start_idx in range(len(pps_samples) - mean_window_size + 1):
        pps_window = pps_samples[start_idx:start_idx + mean_window_size]
        gbps_window = gbps_samples[start_idx:start_idx + mean_window_size]

        pps_mean = np.mean(pps_window)
        pps_std = np.std(pps_window, ddof=1)
        gbps_mean = np.mean(gbps_window)
        gbps_std = np.std(gbps_window, ddof=1)

        pps_rel_std = pps_std / pps_mean if pps_mean > 0 else float('inf')
        gbps_rel_std = gbps_std / gbps_mean if gbps_mean > 0 else float('inf')

        if pps_rel_std <= std_threshold and gbps_rel_std <= std_threshold:
            print(f"  Converged with window at samples {start_idx+1}-{start_idx+mean_window_size}")
            return pps_window, gbps_window

    # If no window converged, return the last mean_window_size samples
    print(f"  No convergence - using last {mean_window_size} samples")
    return pps_samples[-mean_window_size:], gbps_samples[-mean_window_size:]


def generate_message_sizes(min_size, max_size, num_samples):
    """Generate message sizes equidistant in log2 scale"""
    if num_samples == 1:
        return [min_size]

    log2_min = np.log2(min_size)
    log2_max = np.log2(max_size)
    log2_sizes = np.linspace(log2_min, log2_max, num_samples)
    return [int(2 ** log2_size) for log2_size in log2_sizes]


def run_receiver_mode(args):
    """Run as slave (rank 1)"""
    message_sizes = generate_message_sizes(args.min_msg_size, args.max_msg_size, args.num_samples)

    print(f"Running as RECEIVER (rank 1) - {len(message_sizes)} message sizes\n")

    for i, msg_size in enumerate(message_sizes, 1):
        print(f"[{i}/{len(message_sizes)}] Message size: {msg_size}")
        run_benchmark(args.binary, args.config_file, "receiver", msg_size, args.batch_size, args.max_samples)


def run_sender_mode(args):
    """Run as master (rank 0) and collect results"""
    message_sizes = generate_message_sizes(args.min_msg_size, args.max_msg_size, args.num_samples)

    print(f"Running as SENDER (rank 0) - {len(message_sizes)} message sizes\n")

    results = []
    for i, msg_size in enumerate(message_sizes, 1):
        print(f"[{i}/{len(message_sizes)}] Message size: {msg_size}")

        pps_samples, gbps_samples = collect_samples(
            args.binary, args.config_file, "sender", msg_size,
            args.batch_size, args.max_samples, args.mean_window_size, args.std_threshold
        )

        if not pps_samples:
            print(f"  Skipping - no samples\n")
            continue

        mean_pps = np.mean(pps_samples)
        std_pps = np.std(pps_samples, ddof=1)
        mean_gbps = np.mean(gbps_samples)
        std_gbps = np.std(gbps_samples, ddof=1)

        results.append({
            'msg_size': msg_size,
            'mean_pps': mean_pps,
            'std_pps': std_pps,
            'mean_gbps': mean_gbps,
            'std_gbps': std_gbps
        })

        print(f"  Results: PPS={mean_pps:.2f}±{std_pps:.2f}, GBPS={mean_gbps:.2f}±{std_gbps:.2f}\n")

    # Write CSV
    with open(args.output, 'w', newline='') as csvfile:
        writer = csv.DictWriter(csvfile, fieldnames=['msg_size', 'mean_pps', 'std_pps', 'mean_gbps', 'std_gbps'])
        writer.writeheader()
        for result in results:
            writer.writerow({
                'msg_size': result['msg_size'],
                'mean_pps': f"{result['mean_pps']:.2f}",
                'std_pps': f"{result['std_pps']:.2f}",
                'mean_gbps': f"{result['mean_gbps']:.2f}",
                'std_gbps': f"{result['std_gbps']:.2f}"
            })

    print(f"Results written to {args.output}")


def main():
    parser = argparse.ArgumentParser(description='Run RDMA benchmark with varying message sizes')
    parser.add_argument('--binary', type=str, required=True, help='Path to the benchmark binary')
    parser.add_argument('--config-file', type=str, required=True, help='Path to config.json')
    parser.add_argument('--mode', type=str, required=True, choices=['master', 'slave'],
                        help='Run as master (rank 0) or slave (rank 1)')
    parser.add_argument('--batch-size', type=int, default=512, help='Batch size for benchmark')
    parser.add_argument('--min-msg-size', type=int, required=True, help='Minimum message size')
    parser.add_argument('--max-msg-size', type=int, required=True, help='Maximum message size')
    parser.add_argument('--num-samples', type=int, required=True, help='Number of message size samples')
    parser.add_argument('--mean-window-size', type=int, default=10,
                        help='Window size for computing mean/std (default: 10)')
    parser.add_argument('--max-samples', type=int, default=20,
                        help='Maximum samples to collect per message size (default: 20)')
    parser.add_argument('--std-threshold', type=float, default=0.02,
                        help='Relative std threshold for convergence (default: 0.02 = 2%%)')
    parser.add_argument('--output', type=str, default='benchmark_results.csv', help='Output CSV file')

    args = parser.parse_args()

    if args.mode == 'slave':
        run_receiver_mode(args)
    else:
        run_sender_mode(args)


if __name__ == '__main__':
    main()