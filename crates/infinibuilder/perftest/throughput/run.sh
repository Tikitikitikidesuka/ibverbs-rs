#!/bin/bash

# Default values
PYTHON_SCRIPT="./run_throughput.py"
BINARY="./throughput_perftest"
DEVICES_FILE="devices.json"
HOSTFILE="hosts.txt"
RANKFILE="ranks.txt"
BATCH_SIZE=512
MIN_MSG_SIZE=64
MAX_MSG_SIZE=65536
NUM_SAMPLES=10
MEAN_WINDOW_SIZE=10
MAX_SAMPLES=20
STD_THRESHOLD=0.02
OUTPUT_FILE="benchmark_results.csv"
PIPELINE_SIZE=1
PORT=10000

# Function to show usage
usage() {
    echo "Usage: $0 --sender <hostname> --receiver <hostname> [OPTIONS]"
    echo ""
    echo "Required arguments:"
    echo "  --sender <hostname>      Sender machine hostname"
    echo "  --receiver <hostname>    Receiver machine hostname"
    echo ""
    echo "Optional arguments:"
    echo "  --python-script <path>   Path to Python benchmark script (default: ./run_throughput.py)"
    echo "  --binary <path>          Path to benchmark binary (default: ./throughput_perftest)"
    echo "  --devices-file <file>    Devices JSON file (default: devices.json)"
    echo "  --hostfile <file>        MPI hostfile (default: hosts.txt)"
    echo "  --rankfile <file>        MPI rankfile (default: ranks.txt)"
    echo "  --batch-size <N>         Batch size (default: 512)"
    echo "  --min-msg-size <N>       Minimum message size (default: 64)"
    echo "  --max-msg-size <N>       Maximum message size (default: 65536)"
    echo "  --num-samples <N>        Number of message size samples (default: 10)"
    echo "  --mean-window-size <N>   Mean window size (default: 10)"
    echo "  --max-samples <N>        Max samples per message size (default: 20)"
    echo "  --std-threshold <float>  Std convergence threshold (default: 0.02)"
    echo "  --output <file>          Output CSV file (default: benchmark_results.csv)"
    echo "  --pipeline-size <N>      Batch size for transport pipelined operations (default: 1)"
    echo "  --port <N>               Port number (default: 10000)"
    echo ""
    exit 1
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --sender)
            SENDER_HOSTNAME="$2"
            shift 2
            ;;
        --receiver)
            RECEIVER_HOSTNAME="$2"
            shift 2
            ;;
        --python-script)
            PYTHON_SCRIPT="$2"
            shift 2
            ;;
        --binary)
            BINARY="$2"
            shift 2
            ;;
        --devices-file)
            DEVICES_FILE="$2"
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
        --min-msg-size)
            MIN_MSG_SIZE="$2"
            shift 2
            ;;
        --max-msg-size)
            MAX_MSG_SIZE="$2"
            shift 2
            ;;
        --num-samples)
            NUM_SAMPLES="$2"
            shift 2
            ;;
        --mean-window-size)
            MEAN_WINDOW_SIZE="$2"
            shift 2
            ;;
        --max-samples)
            MAX_SAMPLES="$2"
            shift 2
            ;;
        --std-threshold)
            STD_THRESHOLD="$2"
            shift 2
            ;;
        --output)
            OUTPUT_FILE="$2"
            shift 2
            ;;
        --port)
            PORT="$2"
            shift 2
            ;;
        --pipeline-size)
            PIPELINE_SIZE="$2"
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
if [ -z "$SENDER_HOSTNAME" ] || [ -z "$RECEIVER_HOSTNAME" ]; then
    echo "Error: --sender and --receiver are required"
    echo ""
    usage
fi

# Create hostfile
cat > "$HOSTFILE" << EOF
$SENDER_HOSTNAME
$RECEIVER_HOSTNAME
EOF

# Create rankfile
cat > "$RANKFILE" << EOF
rank 0=$SENDER_HOSTNAME slot=0
rank 1=$RECEIVER_HOSTNAME slot=0
EOF

echo "Running RDMA benchmark with MPI"

# Run with MPI - rank 0 is sender, rank 1 is receiver
mpirun -n 2 -bind-to core --hostfile "$HOSTFILE" --rankfile "$RANKFILE" bash -c "if [ \$OMPI_COMM_WORLD_RANK -eq 0 ]; then MODE=sender; else MODE=receiver; fi; python3 $PYTHON_SCRIPT --binary $BINARY --devices-file $DEVICES_FILE --sender-hostname $SENDER_HOSTNAME --receiver-hostname $RECEIVER_HOSTNAME --mode \$MODE --port $PORT --batch-size $BATCH_SIZE --pipeline-size $PIPELINE_SIZE --min-msg-size $MIN_MSG_SIZE --max-msg-size $MAX_MSG_SIZE --num-samples $NUM_SAMPLES --mean-window-size $MEAN_WINDOW_SIZE --max-samples $MAX_SAMPLES --std-threshold $STD_THRESHOLD --output $OUTPUT_FILE"

echo "Test completed. Output saved to $OUTPUT_FILE"
