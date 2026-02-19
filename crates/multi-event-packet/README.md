# 💫🛠️ Multi Event Packet

[![Static Badge](https://img.shields.io/badge/docs-available-blue)](https://lb-rusteb-docs.docs.cern.ch/multi_event_packet)

This crate provides the [`MultiEventPacket`] (MEP, pronounced /mæp/) type used in LHCb event building.

This crate further provides two builder structs to build MEPs from MFPs.
The first one [`SimpleMepBuilder`] is mainly for testing purposes and is easier to use.
The second one [`ZeroCopyMepBuilder`] is for high-performance zero-copy event building.

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

## Example

### Simple Builder

```no_run
# use multi_event_packet::SimpleMepBuilder;
# use ebutils::{odin::dummy_odin_payload, FragmentType, SourceId, SubDetector};
# use multi_fragment_packet::MultiFragmentPacket;
let mfp1: &MultiFragmentPacket = todo!();
let mfp2: &MultiFragmentPacket = todo!();

// 🛠 build
let mep = SimpleMepBuilder::with_capacity(2)
    .add_mfp_ref(mfp1).unwrap()
    .add_mfp_ref(mfp2).unwrap()
    .build().unwrap();

// 💲 profit
for mfp in mep.mfp_iter_srcid_range(SubDetector::Odin.source_id_range()) {
    // do something with mfp
}
```

### Zero Copy Builder

```no_run
# use multi_fragment_packet::MultiFragmentPacket;
# use multi_event_packet::{MultiEventPacket, zerocopy_builder::ZeroCopyMepBuilder};

let mut buffer = vec![0u32; 1024];
let mut mfp_sizes = vec![0usize; 3];
let mut builder = ZeroCopyMepBuilder::new(&mut buffer, &mut mfp_sizes, 4);

// Register the sizes of the MFPs
builder.register_mfp(0, 100);
builder.register_mfp(1, 200);
builder.register_mfp(2, 300);

let mut builder = builder.start_assembling();

// Get the byte slices where each MFP should be stored
let mfp1_slot = builder.get_mfp_slot(0);
let mfp2_slot = builder.get_mfp_slot(1);
let mfp3_slot = builder.get_mfp_slot(2);

let mfps: &[MultiFragmentPacket] = todo!();
// store the MFPs in the slots
mfp1_slot.copy_from_slice(mfps[0].raw_packet_data());
mfp2_slot.copy_from_slice(mfps[1].raw_packet_data());
mfp3_slot.copy_from_slice(mfps[2].raw_packet_data());

// Build the MEP
let mep = builder.finish().expect("Valid MEP");
```

## Features

- `bincode`: [Bincode](https://docs.rs/bincode/latest/bincode/) integration allowing to encode and decode MEPs.
