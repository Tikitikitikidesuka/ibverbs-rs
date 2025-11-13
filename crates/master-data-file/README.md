# 💫🛠️ Master Data File
[![Static Badge](https://img.shields.io/badge/docs-available-blue)](https://lb-rusteb-docs.docs.cern.ch/master_data_file)

This crate allows reading and writing master data files (MDF).

For reading, there exist the [`MdfRecord`] type for accessing a record inside an MDF file (often also just referred to as an MDF).
Open a file consisting of many records, use the [`MdfFile`] struct and its methods. This struct can also be used for "files" that just reside in memory.
To write an MEP to file using the MDF format, use the [`WriteMdf`] extension trait.


## What is an MDF?
MDF is a generic file format for storing event data. An MDF file consists of a sequence of MDF record (often also just called MDF).

In a common case, these records contain the data for a single event from all sources.
Compared to MEPs, which are just a concatenation of MFPs for different sources, each consisting of multiple events, in the MDF the data for a single event is tightly grouped together.
One MDF record inside an MDF file contains data from all the sources of all sub-detectors for a single event after one another.

The MDF format (a bit out of date and with some errors) is defined [here](https://edms.cern.ch/ui/file/784588/2/Online_Raw_Data_Format.pdf#page=5).
For the specialized format, the content of the fragment, called "banks" is described [here](https://edms.cern.ch/ui/file/565851/5/edms-565851.5.pdf#page=10).

## Features
- `mmap`: Allows to mmap MDF files for easier on-demand reading.
- `mep`: Add ability to wriet MEPs to an MDF file.