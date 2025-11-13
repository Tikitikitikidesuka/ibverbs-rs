# 💫🛠️ Rust Event Builder

This collection of crates contains libraries and bindings for building an event builder at [LHCb](https://home.cern/science/experiments/lhcb) at [CERN](https://home.cern).

This repository contains the following sub-crates:
- [`ebutils`](crates/ebutils/README.md): Utilities for aligning memory addresses. Probably can mostly be replaced by rust internal functions.
- [`multi-fragment-packet`](crates/multi-fragment-packet/README.md): Library for reading (and constructing) MFPs.
- [`multi-event-packet`](crates/multi-event-packet/README.md): Library for assembling and reading MEPs.
- [`master-data-file`](crates/master-data-file/README.md): Library for reading and writing MDF files.
- [`pcie40`](crates/pcie40/README.md): Rust bindings for the C driver for the PCIe40 data acquisition card.
- [`shared-memory-buffer`](crates/shared-memory-buffer/README.md)
- [`mock-buffers`](crates/mock-buffers/README.md)
- [`circular-buffer`](crates/circular-buffer/README.md)

Additionally, there are some examples in the [`examples`](examples) directory.

## Overview of data formats and their relationships
todo