# Development Environment Setup Guide

## Overview

The delopment workflow for this project involves:

1. Connecting to the development server via SSH.
2. Setting up a development container with all necessary dependencies.
3. Developing inside of the container environment.

This ensures a consistent development environment with all required tools and dependencies pre-configured.

## Connecting to the Development Server

The development server is currently `tdeb20` at `tdeb20.lbdaq.cern.ch`. To connect to the development machine from outside Point 8 check out the [Connecting from outside Point 8 guide](https://lbtwiki.cern.ch/bin/view/Online/ConnectingFromOutside). Once set up, just connect to your user on the server over SSH, e.g., `ssh keo@tdeb20.lbdaq.cern.ch`.

## Setting up the Development Container

With Docker installed on the development machine (it's actually Podman but it's API compatible), start the rust-dev service specified in the docker-compose.yml file:

```sh
docker-compose up -d rust-dev
```

This will create and start a container with all the required development tools and dependencies for the project.

**Note**: This container includes all _current_ dependencies installed. If new dependencies are added to the project, Cargo may fail to download them due to restrictive internet access on the server. In such cases, the Docker image must be rebuilt and pushed to the registry. Refer to the [docker-setup.md](docker-setup.md) documentation for detailed instructions on this.

## Visual Studio Code Development IDE

A recommended way of setting up a graphical IDE for this development workflow is using Visual Studio Code with two essential extensions:

- [Remote - SSH](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-ssh): Enables you to connect directly to the development server, installing a VS Code Server that provides a seamless development experience with full access to your local IDE features while working on remote code.

- [Dev Containers](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers): Detects the .devcontainer configuration in your project and offers to reopen it inside the container environment, ensuring you're working with all the necessary tools and dependencies.

The workflow is straightforward:

1. Connect to the development server using Remote - SSH
2. VS Code detects the container configuration automatically
3. Reopen the project in the container when prompted
4. Begin development with all dependencies and tools readily available