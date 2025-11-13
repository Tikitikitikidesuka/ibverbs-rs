# 💫🛠️ Rust Event Builder

[![Static Badge](https://img.shields.io/badge/docs-available-blue)](https://lb-rusteb-docs.docs.cern.ch)


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
<!-- To update this, click on the link, edit in browser, and then generate new link under "Actions"-->
[![](https://mermaid.ink/img/pako:eNp1kU1vgzAMhv-K5TOgUCDQXNch7VCp54lL2gSKVkiVj2ob8N8X2FoxTTvaef08tjLgSQmJDKXetbzRvKv6_a6EcQzDcYBS86aTvQUGlU_2pjUWVA1RFEGtNBjeSZC3OfGyA94LEG1dSz03jHL6JH3fVPgNnWboCPvnw8I7uvYioNaqWwK-6x_DELx3Xx7uRuuV5l-ngaOzv6TaraTlinm_ZQ3-w30svRwzx-TJ2fa2MlaIATa6FcisdjLATuqOzyUOVQ-ebs-ykxXOIsH127zJ5GeuvH9VqruPaeWaM7KaX4yv3FVwK3_-4NH1Nwmpn5TrLbI43ZKFgmzAd2SbJI2SzbaIi3yTZUW2TQP8QEZpRPKc0DSL05iSmE4Bfi5eEhVFmhJS5JRQuk2KZPoCe_SiyA?type=png)](https://mermaid.live/edit#pako:eNp1kU1vgzAMhv-K5TOgUCDQXNch7VCp54lL2gSKVkiVj2ob8N8X2FoxTTvaef08tjLgSQmJDKXetbzRvKv6_a6EcQzDcYBS86aTvQUGlU_2pjUWVA1RFEGtNBjeSZC3OfGyA94LEG1dSz03jHL6JH3fVPgNnWboCPvnw8I7uvYioNaqWwK-6x_DELx3Xx7uRuuV5l-ngaOzv6TaraTlinm_ZQ3-w30svRwzx-TJ2fa2MlaIATa6FcisdjLATuqOzyUOVQ-ebs-ykxXOIsH127zJ5GeuvH9VqruPaeWaM7KaX4yv3FVwK3_-4NH1Nwmpn5TrLbI43ZKFgmzAd2SbJI2SzbaIi3yTZUW2TQP8QEZpRPKc0DSL05iSmE4Bfi5eEhVFmhJS5JRQuk2KZPoCe_SiyA)