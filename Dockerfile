FROM almalinux:9

# Install build tools and RDMA/NUMA dependencies
RUN dnf update -y && \
    dnf groupinstall -y "Development Tools" && \
    dnf install -y --allowerasing \
    curl \
    git \
    clang \
    make \
    cmake \
    gcc \
    pkgconf-pkg-config \
    libnl3-devel \
    rdma-core-devel \
    numactl-libs \
    numactl-devel \
    && dnf clean all

# Set environment variables for Rust
ENV RUSTUP_HOME=/opt/rustup
ENV CARGO_HOME=/opt/cargo
ENV PATH="/opt/cargo/bin:${PATH}"
ENV CARGO_TARGET_DIR=/opt/cargo-target

# Install Rust with rustup
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# Install stable rust and additional components
RUN rustup install stable && \
    rustup default stable && \
    rustup component add rust-src rustfmt clippy

# Set working directory to the project dir
WORKDIR /app

# Pre-compile dependencies
COPY Cargo.toml Cargo.lock build.rs ./
RUN mkdir src && echo "" > src/lib.rs && \
    cargo build --lib --all-features 2>/dev/null; \
    cargo build --lib --all-features --release 2>/dev/null; \
    rm -rf src

# Default command
CMD ["/bin/bash"]
