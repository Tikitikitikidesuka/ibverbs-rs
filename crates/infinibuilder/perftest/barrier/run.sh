#!/bin/bash

# Default values
BINARY="./barrier_perftest"
CONFIG="config.json"
HOSTFILE="hosts.txt"
RANKFILE="ranks.txt"
BATCH_SIZE=1024
ALGORITHM="centralized"
ITERS=10

# Function to show usage
usage() {
    echo "Usage: $0 --num-nodes <N> --output <file> [OPTIONS]"
    echo ""
    echo "Required arguments:"
    echo "  --num-nodes <N>        Number of nodes/processes"
    echo "  --output <file>        Output file path"
    echo ""
    echo "Optional arguments:"
    echo "  --binary <path>        Path to barrier_perftest binary (default: ./barrier_perftest)"
    echo "  --config <file>        Config JSON file (default: config.json)"
    echo "  --hostfile <file>      MPI hostfile (default: hosts.txt)"
    echo "  --rankfile <file>      MPI rankfile (default: ranks.txt)"
    echo "  --batch-size <N>       Batch size (default: 1024)"
    echo "  --algorithm <name>     Algorithm name (default: centralized)"
    echo "  --iters <N>            Number of iterations (default: 10)"
    echo ""
    exit 1
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --num-nodes)
            NUM_NODES="$2"
            shift 2
            ;;
        --output)
            OUTPUT_FILE="$2"
            shift 2
            ;;
        --binary)
            BINARY="$2"
            shift 2
            ;;
        --config)
            CONFIG="$2"
            shift 2
            ;;
        --hostfile)
            HOSTFILE="$2"
            shift 2
            ;;
        --rankfile)
            RANKFILE="$2"
            shift 2
            ;;
        --batch-size)
            BATCH_SIZE="$2"
            shift 2
            ;;
        --algorithm)
            ALGORITHM="$2"
            shift 2
            ;;
        --iters)
            ITERS="$2"
            shift 2
            ;;
        -h|--help)
            usage
            ;;
        *)
            echo "Unknown option: $1"
            usage
            ;;
    esac
done

# Check required arguments
if [ -z "$NUM_NODES" ] || [ -z "$OUTPUT_FILE" ]; then
    echo "Error: --num-nodes and --output are required"
    echo ""
    usage
fi

# Run the test
mpirun -n $NUM_NODES -bind-to core --hostfile "$HOSTFILE" --rankfile "$RANKFILE" bash -c "$BINARY --config-file $CONFIG --batch-size $BATCH_SIZE --num-nodes \$OMPI_COMM_WORLD_SIZE --rank-id \$OMPI_COMM_WORLD_RANK --algorithm $ALGORITHM --iters $ITERS" > "$OUTPUT_FILE"

echo "Test completed. Output saved to $OUTPUT_FILE"
