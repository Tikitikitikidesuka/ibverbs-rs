# Rust Event Builder

This is a work in progress Rust version of the Event Builder for [LHCb](https://home.cern/science/experiments/lhcb)
at [CERN](https://home.cern).

The main crate in this workspace defines libraries to coordinate and integrate the sub-crate components used for event
building, as well as a binary that runs the Event Builder process itself.

The sub-crates represent the major components of the Event Builder:

- [`pcie40`](crates/pcie40/): Rust bindings for the C driver of the PCIe40 data acquisition card.
- [`shared-memory-buffer`](crates/shared-memory-buffer/): Shared memory transport for inter-process data exchange.
  Compatible with LHCb’s established shared memory protocol and designed to serve as a drop-in replacement.
- [`mock-buffers`](crates/mock-buffers/): TODO: Remove this crate and move each mock buffer to its corresponding crate.
- [`circular-buffer`](crates/circular-buffer/): Trait defining circular buffer abstractions for interoperability between
  `pcie40` buffers and `shared-memory` buffers.
- [`ebutils`](crates/ebutils/): Utilities for aligning memory addresses. Probably can mostly be replaced by rust
  internal functions.
- [`multi-fragment-packet`](crates/multi-fragment-packet/): Library for reading and constructing MFPs.
- [`multi-event-packet`](crates/multi-event-packet/): Library for assembling and reading MEPs.
- [`master-data-file`](crates/master-data-file/): Library for reading and writing MDF files.

Additionally, there are some examples in the [`examples`](examples) directory.