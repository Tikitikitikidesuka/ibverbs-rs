FROM --platform=linux/amd64 gitlab-registry.cern.ch/linuxsupport/alma9-base:latest

# Set up environment variables
ENV HOME /root
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

# Install PCIe40 packages and documentation
RUN dnf update -y && \
    dnf install -y \
    lhcb-pcie40-tools \
    lhcb-pcie40-driver

# Install Rust using rustup
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Create a working directory
WORKDIR /app

# Set default command
CMD ["/bin/bash"]