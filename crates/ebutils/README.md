# 💫🛠️ EB Utils
[![Static Badge](https://img.shields.io/badge/docs-available-blue)](https://lb-rusteb-docs.docs.cern.ch/ebutils)

This crate contains various utilities for the LHCb event building with rust.
In particular, it contains definitions for 
- [`SourceId`]s including typed [`SubDetector`]s,
- [`Fragment`]s with [`FragmentType`] enum, and
- [`OdinPayload`]s for Odin fragments, including a builder.

Furthermore, it contains some functions to calculate address alignments in [`alignment`].
