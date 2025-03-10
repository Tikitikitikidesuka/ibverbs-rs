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

# Display script banner
echo "====================================="
echo "Rust Project Build & Deploy Automation"
echo "====================================="

# Check for SSH key
if [ ! -f ~/.ssh/id_rsa ]; then
    echo "No SSH key found. Setting up SSH key for server access..."
    ssh-keygen -t rsa -b 4096 -f ~/.ssh/id_rsa -N ""
    echo "Please copy this SSH key to your server:"
    cat ~/.ssh/id_rsa.pub
    echo "Run: ssh-copy-id $SERVER"
    exit 1
fi

# Step 1: Build the Docker image if it doesn't exist
echo "[1/4] Checking Docker image..."
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
echo "[2/4] Building Rust project inside Docker container..."
docker run --platform=linux/amd64 --rm -v "$(pwd)":/app $DOCKER_IMAGE_NAME:$DOCKER_TAG cargo build

# Check if binary was created
if [ ! -f "./target/release/$BINARY_NAME" ]; then
    echo "Error: Binary was not created. Check the build logs above."
    exit 1
fi

echo "Build successful!"

# Step 3: Copy the binary to the server
echo "[3/4] Copying binary to server..."
ssh "$SERVER" "mkdir -p $SERVER_PATH"
scp "./target/release/$BINARY_NAME" "$SERVER:$SERVER_PATH/$BINARY_NAME"

if [ $? -ne 0 ]; then
    echo "Error: Failed to copy binary to server. Make sure SSH is properly configured."
    exit 1
fi

echo "Binary successfully copied to server!"

# Step 4: Run the binary on the server (optional)
echo "[4/4] Running binary on server..."

# Default run command if not set in config
if [ -z "$RUN_COMMAND" ]; then
    RUN_COMMAND="cd $SERVER_PATH && ./$BINARY_NAME"
fi

ssh "$SERVER" "$RUN_COMMAND"

if [ $? -ne 0 ]; then
    echo "Error: Failed to run binary on server."
    exit 1
fi

echo "====================================="
echo "Deployment completed successfully!"
echo "====================================="