# Docker Image Building Guide

## Authentication

Before pushing or pulling images to the CERN GitLab Container Registry, you must authenticate:

```sh
docker login gitlab-registry.cern.ch
```

**Note:** If you have [Two-Factor Authentication](https://gitlab.cern.ch/help/user/profile/account/two_factor_authentication) enabled, use a [personal access token](https://gitlab.cern.ch/help/user/profile/personal_access_tokens) instead of your password.

## Building and Uploading Images

### Standard Build (x86_64/amd64 Systems)

On a machine with internet access build and push the image to the project's registry:

```sh
# Build the image
docker build -t gitlab-registry.cern.ch/mhermoso/pcie40-rs .

# Push the image to the registry
docker push gitlab-registry.cern.ch/mhermoso/pcie40-rs
```

### Cross-Platform Building (ARM-based Systems)

If you are working on an ARM-based system (e.g., Apple Silicon MacBook) and want to avoid Docker Desktop (which requires a commercial license for business use), an alternative is to use Colima with QEMU:

```sh
# Create a dedicated Colima profile with QEMU x86_64 backend
colima start --profile qemu_x86_64 --arch x86_64_v2

# Build and push as normal
docker build --platform linux/amd64 -t gitlab-registry.cern.ch/mhermoso/pcie40-rs .
docker push gitlab-registry.cern.ch/mhermoso/pcie40-rs
```

This creates a new Colima profile with the appropriate backend for x86_64 architecture emulation.