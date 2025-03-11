#!/bin/bash

# Default server configuration (will be overridden by config file if it exists)
SERVER="user@your-server-hostname"
SERVER_PATH="/path/to/deployment/directory"

# Project constants configuration (not to be changed by users)
BINARY_NAME="pcie40-rs"
DOCKERFILE_PATH=".devcontainer/Dockerfile"
DOCKER_IMAGE_NAME="pcie40-rust-dev"
DOCKER_TAG="latest"

# Load configuration from external file if it exists
CONFIG_FILE="deploy_config.local"
if [ -f "$CONFIG_FILE" ]; then
    echo "Loading configuration from $CONFIG_FILE"
    source "$CONFIG_FILE"
fi

# Function to show usage information
show_usage() {
    echo "Usage: $0 <command> [options]"
    echo ""
    echo "Commands:"
    echo "  build    Build the project in the Docker container"
    echo "  deploy   Copy the binary to the server (without building)"
    echo "  run      Run the binary on the server (without building or deploying)"
    echo "  all      Build, deploy, and run the binary on the server"
    echo "  help     Show this help message"
    echo ""
    echo "Options:"
    echo "  --debug         Build in debug mode (default)"
    echo "  --release       Build in release mode"
    echo ""
    echo "Examples:"
    echo "  $0 build               # Build in debug mode"
    echo "  $0 build --release     # Build in release mode"
    echo "  $0 deploy --release    # Deploy the release binary (assumes it's already built)"
    echo "  $0 run                 # Run the binary on server (assumes it's already deployed)"
    echo "  $0 all --release       # Build in release mode, deploy and run"
    exit 0
}

# Check for command
if [ $# -eq 0 ]; then
    show_usage
    exit 1
fi

COMMAND=$1
shift  # Remove the command from arguments

# Default build mode
BUILD_MODE="debug"
CARGO_ARGS=""

# Parse remaining arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        --debug)
            BUILD_MODE="debug"
            CARGO_ARGS=""
            shift
            ;;
        --release)
            BUILD_MODE="release"
            CARGO_ARGS="--release"
            shift
            ;;
        *)
            echo "Unknown option: $1"
            show_usage
            exit 1
            ;;
    esac
done

# Determine the correct binary path based on build mode
if [ "$BUILD_MODE" = "release" ]; then
    BINARY_PATH="./target/release/$BINARY_NAME"
else
    BINARY_PATH="./target/debug/$BINARY_NAME"
fi

# Default run command if not set in config
if [ -z "$RUN_COMMAND" ]; then
    REMOTE_COMMAND="cd $SERVER_PATH && ./$BINARY_NAME"
else
    REMOTE_COMMAND="$RUN_COMMAND"
fi

# Parse command - each command does exactly one thing
case $COMMAND in
    build)
        DO_BUILD=true
        ;;
    deploy)
        DO_DEPLOY=true
        ;;
    run)
        DO_RUN=true
        ;;
    all)
        DO_BUILD=true
        DO_DEPLOY=true
        DO_RUN=true
        ;;
    help)
        show_usage
        ;;
    *)
        echo "Unknown command: $COMMAND"
        show_usage
        exit 1
        ;;
esac

# Display script banner
echo "====================================="
echo "Rust Project Build & Deploy Automation"
echo "====================================="
echo "Command: $COMMAND (mode: $BUILD_MODE)"
echo "====================================="

# Check for SSH key if needed for deploy or run
if [ "$DO_DEPLOY" = true ] || [ "$DO_RUN" = true ]; then
    if [ ! -f ~/.ssh/id_rsa ]; then
        echo "No SSH key found. Setting up SSH key for server access..."
        ssh-keygen -t rsa -b 4096 -f ~/.ssh/id_rsa -N ""
        echo "Please copy this SSH key to your server:"
        cat ~/.ssh/id_rsa.pub
        echo "Run: ssh-copy-id $SERVER"
        exit 1
    fi
fi

# Build step
if [ "$DO_BUILD" = true ]; then
    echo "[Step] Building project..."
    # Step 1: Build the Docker image if it doesn't exist
    echo "Checking Docker image..."
    if [[ "$(docker images -q $DOCKER_IMAGE_NAME:$DOCKER_TAG 2> /dev/null)" == "" ]]; then
        echo "Building Docker image..."

        # Check if Dockerfile exists
        if [ ! -f "$DOCKERFILE_PATH" ]; then
            echo "Error: Dockerfile not found at $DOCKERFILE_PATH"
            exit 1
        fi

        docker build -t $DOCKER_IMAGE_NAME:$DOCKER_TAG -f "$DOCKERFILE_PATH" .
    fi

    # Step 2: Run the container to build the Rust project
    echo "Building Rust project inside Docker container (mode: $BUILD_MODE)..."
    docker run --platform=linux/amd64 --rm -v "$(pwd)":/app $DOCKER_IMAGE_NAME:$DOCKER_TAG cargo build ${CARGO_ARGS:-}

    # Check if binary was created
    if [ ! -f "$BINARY_PATH" ]; then
        echo "Error: Binary was not created at $BINARY_PATH. Check the build logs above."
        exit 1
    fi

    echo "Build successful!"
fi

# Deploy step
if [ "$DO_DEPLOY" = true ]; then
    echo "[Step] Deploying binary..."

    # Check if binary exists before trying to deploy
    if [ ! -f "$BINARY_PATH" ]; then
        echo "Error: Binary not found at $BINARY_PATH. Run build first or check build mode."
        exit 1
    fi

    # Copy the binary to the server
    echo "Copying binary to server..."
    ssh "$SERVER" "mkdir -p $SERVER_PATH"
    scp "$BINARY_PATH" "$SERVER:$SERVER_PATH/$BINARY_NAME"

    if [ $? -ne 0 ]; then
        echo "Error: Failed to copy to server. Make sure SSH is properly configured."
        exit 1
    fi

    echo "Binary successfully copied to server!"
fi

# Run step
if [ "$DO_RUN" = true ]; then
    echo "[Step] Running binary on server..."

    # Run the binary on the server
    echo "Executing on server..."
    ssh "$SERVER" "$REMOTE_COMMAND"

    if [ $? -ne 0 ]; then
        echo "Error: Failed to run binary on server."
        exit 1
    fi

    echo "Execution completed!"
fi

echo "====================================="
if [ "$DO_BUILD" = true ] && [ "$DO_DEPLOY" = true ] && [ "$DO_RUN" = true ]; then
    echo "Build, deployment, and execution completed successfully!"
elif [ "$DO_BUILD" = true ] && [ "$DO_DEPLOY" = true ]; then
    echo "Build and deployment completed successfully!"
elif [ "$DO_BUILD" = true ]; then
    echo "Build completed successfully!"
elif [ "$DO_DEPLOY" = true ]; then
    echo "Deployment completed successfully!"
elif [ "$DO_RUN" = true ]; then
    echo "Execution completed successfully!"
fi
echo "====================================="