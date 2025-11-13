# Rust Event Builder

This collection of crates contains libraries and bindings for building an event builder at [LHCb](https://home.cern/science/experiments/lhcb) at [CERN](https://home.cern).

This repository contains the following sub-crates:
- [`pcie40`](crates/pcie40/): Rust bindings for the C driver for the PCIe40 data acquisition card.
- [`shared-memory-buffer`](crates/shared-memory-buffer/)
- [`mock-buffers`](crates/mock-buffers/)
- [`circular-buffer`](crates/circular-buffer/)
- [`ebutils`](crates/ebutils/): Utilities for aligning memory addresses. Probably can mostly be replaced by rust internal functions.
- [`multi-fragment-packet`](crates/multi-fragment-packet/): Library for reading (and constructing) MFPs.
- [`multi-event-packet`](crates/multi-event-packet/): Library for assembling and reading MEPs.
- [`master-data-file`](crates/master-data-file/): Library for reading and writing MDF files.

Additionally, there are some examples in the [`examples`](examples) directory.