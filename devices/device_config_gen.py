import json
import random
import argparse
from typing import Optional
from collections import defaultdict


def transform_hosts(
        input_file: str,
        output_file: str,
        hostfile_path: str,
        rankfile_path: str,
        starting_port: int = 10000,
        world_size: Optional[int] = None,
        allow_repetition: bool = False,
        shuffle: bool = False,
        seed: Optional[int] = None,
) -> None:
    """Expand a host list and generate synchronized JSON, Hostfile, and Rankfile.

    All three files are generated from the exact same shuffled population to ensure consistency.
    """

    # 1. Load Input
    with open(input_file, "r", encoding="utf-8") as f:
        data = json.load(f)

    if "hosts" not in data or not isinstance(data["hosts"], list):
        raise ValueError("Input JSON must contain a top-level 'hosts' array")

    input_hosts = data["hosts"]
    n = len(input_hosts)
    if n == 0:
        raise ValueError("Input 'hosts' array is empty")

    # 2. Validate Parameters
    if world_size is None:
        world_size = n
    if world_size < 0:
        raise ValueError("world_size must be >= 0")

    if world_size > n and not allow_repetition:
        raise ValueError(
            f"world_size ({world_size}) > input hosts ({n}). Use --allow-repetition to enable cycling."
        )

    # 3. Build Population (Expansion)
    # We build the full list of N=world_size entries first.
    population = []
    emitted = 0
    rep = 0

    # Continue adding passes until we have enough entries
    while emitted < world_size:
        port = starting_port + rep
        for h in input_hosts:
            if emitted >= world_size:
                break
            population.append({
                "hostname": h["hostname"],
                "port": port,
                "ibdev": h["ibdev"]
            })
            emitted += 1
        rep += 1

    # 4. Global Shuffle
    # This ensures that if we pick a random subset (or random order),
    # the same randomization applies to all output files.
    if shuffle:
        rng = random.Random(seed)
        rng.shuffle(population)

    # 5. Generate Outputs
    json_hosts = []
    rankfile_lines = []
    host_slot_counters = defaultdict(int)

    for rank, item in enumerate(population):
        hostname = item["hostname"]

        # Track slot index for this specific host instance
        # (e.g., if tdeb01 appears 3 times, slots will be 0, 1, 2)
        slot_idx = host_slot_counters[hostname]
        host_slot_counters[hostname] += 1

        # Add to JSON list
        # Order: hostname, port, ibdev, rankid
        entry = {
            "hostname": hostname,
            "port": item["port"],
            "ibdev": item["ibdev"],
            "rankid": rank
        }
        json_hosts.append(entry)

        # Add to Rankfile lines
        # Format: rank <id>=<hostname> slot=<slot_idx>
        rankfile_lines.append(f"rank {rank}={hostname} slot={slot_idx}")

    # 6. Write JSON Output
    with open(output_file, "w", encoding="utf-8") as f:
        json.dump({"hosts": json_hosts}, f, indent=2)
        f.write("\n")

    # 7. Write Rankfile
    with open(rankfile_path, "w", encoding="utf-8") as f:
        f.write("\n".join(rankfile_lines))
        f.write("\n")

    # 8. Write Hostfile
    # Format: <hostname> slots=<total_count>
    # Sorted alphabetically for cleanliness
    with open(hostfile_path, "w", encoding="utf-8") as f:
        for hostname in sorted(host_slot_counters.keys()):
            count = host_slot_counters[hostname]
            f.write(f"{hostname} slots={count}\n")

    print(f"Generated {len(json_hosts)} entries:")
    print(f"  JSON:     {output_file}")
    print(f"  Hostfile: {hostfile_path}")
    print(f"  Rankfile: {rankfile_path}")


def main() -> None:
    p = argparse.ArgumentParser(description="Generate consistent JSON, Hostfile, and Rankfile for MPI.")
    p.add_argument("input_file", help="Input JSON file path")
    p.add_argument("output_file", help="Output JSON file path")
    p.add_argument("--hostfile", required=True, help="Output MPI hostfile path")
    p.add_argument("--rankfile", required=True, help="Output MPI rankfile path")

    p.add_argument("--starting-port", type=int, default=10000, help="Base port number (default: 10000)")
    p.add_argument("--world-size", type=int, default=None, help="Total ranks to generate (default: input count)")
    p.add_argument("--allow-repetition", action="store_true", help="Enable recycling hosts (increments ports)")
    p.add_argument("--shuffle", action="store_true", help="Randomize rank placement")
    p.add_argument("--seed", type=int, default=None, help="Random seed for reproducibility")

    args = p.parse_args()

    transform_hosts(
        input_file=args.input_file,
        output_file=args.output_file,
        hostfile_path=args.hostfile,
        rankfile_path=args.rankfile,
        starting_port=args.starting_port,
        world_size=args.world_size,
        allow_repetition=args.allow_repetition,
        shuffle=args.shuffle,
        seed=args.seed,
    )


if __name__ == "__main__":
    main()