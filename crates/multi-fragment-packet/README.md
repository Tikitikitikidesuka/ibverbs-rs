# 💫🛠️ Multi Fragment Packet

This crate provides the [`MultiFragmentPacket`] (MFP) type used in LHCb data acquisition and event building.

This crate further provides interfacing code to allow reading MFPs from the PCIe40 cards (see also [pcei40](../pcie40/README.md)) and from shared memory buffers (see also [shared-memory-buffer](../shared-memory-buffer/README.md)), each requiering a feature flag.

Additionally, a builder for conveniently constructing MFPs for testing purposes is provided.

## Features
- `pcie40-io`: PCIe40 integration
- `shmem-io`: Shared memory integration
- `bincode`: [Bincode](https://docs.rs/bincode/latest/bincode/) integration allowing to encode and decode MFPs.