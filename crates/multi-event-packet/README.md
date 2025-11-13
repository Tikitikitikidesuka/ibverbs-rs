# 💫🛠️ Multi Event Packet
This crate provides the [`MultiEventPacket`] (MEP, pronounced /mæp/) type used in LHCb event building.

This crate further provides a [`MultiEventPacketBuilder`] to build MEPs from MFPs.

# What is an MEP?
A multi event packet (MEP, pronounced /mæp/) is just the concatenation of multiple multi fragment packets (MFPs)
from different sources for one "block" of events
(remember, an MFP contains multiple fragments of some physics data from a single source for some contiguous events).

Each MEP contains exactly one MFP from an ODIN instance, containing metadata for the events (see [`ebutils::OdinPayload`]), like timing information.
The MFPs inside an MEP are ordered by source ID for convenience.
As ODIN MFPs have the sub-detector part of their source ID equal zero, the first MFP of an MEP is always an ODIN MFP.

Constructing MEPs from MFPs is called "event building".
To understand this process, one analogy is to consider a video made up of multiple frames each consisting of multiple pixels.
An MFP contains the same "pixel" for consecutive "frames".
During event building, all the different "pixels" (fragments) coming from the different parts of the sub-detectors (different source IDs) are assembled into one "frame" (event).
To be precise, an MEP contains the data for multiple "frames", as it is just a concatenation of multiple MFPs which contain "pixels" for multiple consecutive "frames".
The advantage of this not yet completely sorted format (having just the data for each individual "frame" together) is that it is faster to construct from MFPs---they only need to be copied over as whole chunks.

The MEP format is defined [here](https://edms.cern.ch/ui/file/2100937/5/edms_2100937_raw_data_format_run3.pdf#section.4).

## Features
- `bincode`: [Bincode](https://docs.rs/bincode/latest/bincode/) integration allowing to encode and decode MEPs.