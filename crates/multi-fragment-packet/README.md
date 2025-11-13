# 💫🛠️ Multi Fragment Packet
[![Static Badge](https://img.shields.io/badge/docs-available-brightgreen)](https://lb-rusteb-docs.docs.cern.ch/multi_fragment_packet)

This crate provides the [`MultiFragmentPacket`] (MFP) type used in LHCb data acquisition and event building.

This crate further provides interfacing code to allow reading MFPs from the PCIe40 cards (see also [pcei40](../pcie40/README.md)) and from shared memory buffers (see also [shared-memory-buffer](../shared-memory-buffer/README.md)), each requiring a feature flag.

Additionally, a builder for conveniently constructing MFPs for testing purposes is provided.

For how to read MFPs from the PCIe40 card, see one of the examples under the workspace root directory.

## What is an MFP?
A multi fragment packet (MFP; not to be confused with MEP) is a collection of fragments, pieces of some physics data, from one specific part of a sub-detector at LHCb and multiple collision events.
An MFP thus contains fragments for the same [`SourceId`](ebutils::SourceId) and multiple consecutive [`EventId`](ebutils::EventId)s.

MFPs are obtained from the PCIe40 readout cards, which are connected to the detector using optical fiber.

The MFP format is defined [here](https://edms.cern.ch/ui/file/2100937/5/edms_2100937_raw_data_format_run3.pdf#section.3).

## Features
- `pcie40-io`: PCIe40 integration
- `shmem-io`: Shared memory integration
- `bincode`: [Bincode](https://docs.rs/bincode/latest/bincode/) integration allowing to encode and decode MFPs.