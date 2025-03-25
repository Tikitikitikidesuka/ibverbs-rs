FROM --platform=linux/amd64 gitlab-registry.cern.ch/linuxsupport/alma9-base:latest

# Set up environment variables
ENV LANG en_US.UTF-8
ENV LC_ALL en_US.UTF-8

# Fix locale issues
RUN dnf install -y glibc-all-langpacks && \
    dnf reinstall -y glibc-common && \
    dnf clean all

# Install development tools and dependencies
RUN dnf update -y && \
    dnf groupinstall -y "Development Tools" && \
    dnf install -y \
    curl \
    wget \
    git \
    vim \
    clang \
    cmake \
    make \
    gcc \
    gcc-c++ \
    kernel-devel \
    kernel-headers \
    pciutils \
    usbutils \
    openssl-devel && \
    dnf clean all

# Install EPEL repository
RUN dnf install -y epel-release && \
    dnf update -y

# Add DAQ40 repo configuration
RUN mkdir -p /etc/yum.repos.d/ && \
    echo -e "[daq40-software-stable]\nname=DAQ40 stable packages for \$basearch\nbaseurl=https://lhcb-online-soft.web.cern.ch/rpm/daq/stable/el\$releasever/\$basearch\nenabled=1\ngpgcheck=0\nprotect=1" > /etc/yum.repos.d/daq40.repo

# Install PCIe40 packages
RUN dnf update -y && \
    dnf install -y \
    lhcb-pcie40-tools \
    lhcb-pcie40-driver

# Create a non-root user
RUN useradd -m -s /bin/bash developer && \
    echo "developer ALL=(ALL) NOPASSWD: ALL" > /etc/sudoers.d/developer

# Set environment variables for Rust
ENV RUSTUP_HOME=/opt/rustup
ENV CARGO_HOME=/opt/cargo
ENV PATH="/opt/cargo/bin:${PATH}"

# Install Rust with explicit source components
RUN mkdir -p ${RUSTUP_HOME} ${CARGO_HOME} && \
    chmod -R 777 ${RUSTUP_HOME} ${CARGO_HOME} && \
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path && \
    rustup install stable && \
    rustup default stable && \
    rustup component add rust-src rustfmt clippy && \
    chmod -R 777 ${RUSTUP_HOME} ${CARGO_HOME}

# Create working directory and set permissions
WORKDIR /app
RUN mkdir -p /app && chown -R developer:developer /app

# Switch to non-root user for the rest of the build
USER developer

# Copy only the dependency manifests (if they exist at build time)
COPY --chown=developer:developer Cargo.toml* ./

# Create a minimal src/main.rs to trick cargo into downloading dependencies
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs

# Download and build dependencies only (with error handling)
RUN cargo fetch || echo "Cargo fetch step skipped or failed" && \
    cargo build --release || cargo build



# Create a working directory
WORKDIR /app

# Copy only the dependency manifests
COPY Cargo.toml ./

# Create a minimal src/main.rs to trick cargo into downloading dependencies
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs

# Download and build dependencies only
RUN cargo build --release || cargo build

# This image won't contain the project source code
# The source should be mounted when running the container
# For example:
# podman run -v $(pwd):/app/src -it your-image-name

# Set default command
CMD ["/bin/bash"]