# 💫🛠️ Rust Event Builder

[![Static Badge](https://img.shields.io/badge/docs-available-blue)](https://lb-rusteb-docs.docs.cern.ch)


This is a work in progress Rust version of the Event Builder for [LHCb](https://home.cern/science/experiments/lhcb)
at [CERN](https://home.cern).

The main crate in this workspace defines libraries to coordinate and integrate the sub-crate components used for event
building, as well as a binary that runs the Event Builder process itself.

The sub-crates represent the major components of the Event Builder:

- [`pcie40`](crates/pcie40/): Rust bindings for the C driver of the PCIe40 data acquisition card.
- [`shared-memory-buffer`](crates/shared-memory-buffer/): Shared memory transport for inter-process data exchange.
  Compatible with LHCb’s established shared memory protocol and designed to serve as a drop-in replacement.
- [`mock-buffers`](crates/mock-buffers/): Mock buffers with the `circular-buffer` traits for testing purposes.
- [`circular-buffer`](crates/circular-buffer/): Trait defining circular buffer abstractions for interoperability between
  `pcie40` buffers and `shared-memory` buffers.
- [`ebutils`](crates/ebutils/): Utility crate containing definitions for common types like source IDs and fragments.
- [`multi-fragment-packet`](crates/multi-fragment-packet/): Library for reading and constructing MFPs.
- [`multi-event-packet`](crates/multi-event-packet/): Library for assembling and reading MEPs.
- [`master-data-file`](crates/master-data-file/): Library for reading and writing MDF files.

Additionally, there are some examples in the [`examples`](examples) directory.

## Overview of data formats and their relationships
- **Fragment**: Just some detector specific data for some event.
- **Multi Fragment Packet** *(MFP)*: Multiple Fragments from the same source (detector part) for consecutive events.
- **Multi Event Packet** *(MEP, pronounced /mæp/)*: Multiple MFPs for the same events but from different sources concatenated
- **Master Data File** *(MDF)*: File format that stores all the fragments for one event together. An MDF file contains many MDF records for many events.

<!-- To update this, click on the link, edit in browser, and then generate new link under "Actions". Make sure to change `img` to `svg` in the path to get an svg image.-->
[![](https://mermaid.ink/svg/pako:eNp1kU1vgzAMhv-K5TOgUCDQXNch7VCp54lL2gSKVkiVj2ob8N8X2FoxTTvaef08tjLgSQmJDKXetbzRvKv6_a6EcQzDcYBS86aTvQUGlU_2pjUWVA1RFEGtNBjeSZC3OfGyA94LEG1dSz03jHL6JH3fVPgNnWboCPvnw8I7uvYioNaqWwK-6x_DELx3Xx7uRuuV5l-ngaOzv6TaraTlinm_ZQ3-w30svRwzx-TJ2fa2MlaIATa6FcisdjLATuqOzyUOVQ-ebs-ykxXOIsH127zJ5GeuvH9VqruPaeWaM7KaX4yv3FVwK3_-4NH1Nwmpn5TrLbI43ZKFgmzAd2SbJI2SzbaIi3yTZUW2TQP8QEZpRPKc0DSL05iSmE4Bfi5eEhVFmhJS5JRQuk2KZPoCe_SiyA?type=png)](https://mermaid.live/edit#pako:eNp1kU1vgzAMhv-K5TOgUCDQXNch7VCp54lL2gSKVkiVj2ob8N8X2FoxTTvaef08tjLgSQmJDKXetbzRvKv6_a6EcQzDcYBS86aTvQUGlU_2pjUWVA1RFEGtNBjeSZC3OfGyA94LEG1dSz03jHL6JH3fVPgNnWboCPvnw8I7uvYioNaqWwK-6x_DELx3Xx7uRuuV5l-ngaOzv6TaraTlinm_ZQ3-w30svRwzx-TJ2fa2MlaIATa6FcisdjLATuqOzyUOVQ-ebs-ykxXOIsH127zJ5GeuvH9VqruPaeWaM7KaX4yv3FVwK3_-4NH1Nwmpn5TrLbI43ZKFgmzAd2SbJI2SzbaIi3yTZUW2TQP8QEZpRPKc0DSL05iSmE4Bfi5eEhVFmhJS5JRQuk2KZPoCe_SiyA)