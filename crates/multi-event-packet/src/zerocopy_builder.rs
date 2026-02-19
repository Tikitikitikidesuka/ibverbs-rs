//! This module provides the zero-copy [`ZeroCopyMepBuilder`] to build [`MultiEventPacket`]s out of [`MultiFragmentPacket`]s used for high-performance event building.
//!
//! For testing purpouses, consider using the simpler [`SimpleMepBuilder`](crate::builder::SimpleMepBuilder).

use std::{num::NonZero, ops::Range};

use bytemuck::{cast_slice, cast_slice_mut};
use multi_fragment_packet::{FromRawBytesError, MultiFragmentPacket};

use crate::{
    MultiEventPacket, MultiEventPacketConstHeader,
    simple_builder::{
        access_offsets, offsets_iter, write_const_header, write_offsets, write_source_ids,
    },
    total_header_size,
    zerocopy_builder::internal::Stage,
};

mod internal {
    pub(super) trait Stage {}
}

/// First of two stages of the [`ZeroCopyMepBuilder`].
///
/// In this stage, you register the sizes of all the MFPs that will be included in the MEP.
pub struct RegisterSizes {}
impl Stage for RegisterSizes {}

/// Second of two stages of the [`ZeroCopyMepBuilder`].
///
/// In this stage, you can get byte slices where each mfp should be stored in the final MEP buffer.
pub struct StoreMfps {
    /// in bytes
    total_size: usize,
}
impl Stage for StoreMfps {}

/// This is a builder struct to construct [`MultiEventPacket`]s from [`MultiFragmentPacket`]s.
///
/// Although a bit more cumbersome to use than [`SimpleMepBuilder`](crate::builder::SimpleMepBuilder), it is suitable for high-performance zero-copy applications.
/// After registering the sizes of all the MFPs, you can get byte slices where each mfp should be stored in the final MEP buffer.
///
/// # Example
/// ```no_run
/// # use multi_fragment_packet::MultiFragmentPacket;
/// # use multi_event_packet::{MultiEventPacket, zerocopy_builder::ZeroCopyMepBuilder};
///
/// let mut buffer = vec![0u32; 1024];
/// let mut mfp_sizes = vec![0usize; 3];
/// let mut builder = ZeroCopyMepBuilder::new(&mut buffer, &mut mfp_sizes, 4);
///
/// // Register the sizes of the MFPs
/// builder.register_mfp(0, 100);
/// builder.register_mfp(1, 200);
/// builder.register_mfp(2, 300);
///
/// let mut builder = builder.start_assembling();
///
/// // Get the byte slices where each MFP should be stored
/// let mfp1_slot = builder.get_mfp_slot(0);
/// let mfp2_slot = builder.get_mfp_slot(1);
/// let mfp3_slot = builder.get_mfp_slot(2);
///
/// let mfps: &[MultiFragmentPacket] = todo!();
///
/// // store the MFPs in the slots
/// mfp1_slot.copy_from_slice(mfps[0].raw_packet_data());
/// mfp2_slot.copy_from_slice(mfps[1].raw_packet_data());
/// mfp3_slot.copy_from_slice(mfps[2].raw_packet_data());
///
/// // Build the MEP
/// let mep = builder.finish().expect("Valid MEP");
/// ```
#[allow(private_bounds)]
pub struct ZeroCopyMepBuilder<'a, S: Stage> {
    buffer: &'a mut [u32],
    mfp_sizes_bytes: &'a mut [Option<NonZero<usize>>],
    mfp_align: usize,
    stage: S,
}

#[allow(private_bounds)]
impl<'a, S: Stage> ZeroCopyMepBuilder<'a, S> {
    /// Returns a pointer range of the underlying buffer.
    pub fn get_buffer_range(&mut self) -> Range<*mut u32> {
        self.buffer.as_mut_ptr_range()
    }

    /// Returns the number of MFPs this event builder is created for.
    pub fn num_mfps(&self) -> usize {
        self.mfp_sizes_bytes.len()
    }
}

impl<'a> ZeroCopyMepBuilder<'a, RegisterSizes> {
    /// Creates a new `ZeroCopyMepBuilder` with the given buffer and MFP size cache.
    ///
    /// Length of mfp_size_cache must match the number of MFPs to construct mep for.
    ///
    /// The MFPs inside the constructed MEP will be aligned to `mfp_align` bytes.
    pub fn new(buffer: &'a mut [u32], mfp_size_cache: &'a mut [usize], mfp_align: usize) -> Self {
        let mfp_sizes_bytes = bytemuck::cast_slice_mut(mfp_size_cache);
        mfp_sizes_bytes.fill(None);

        ZeroCopyMepBuilder {
            buffer,
            mfp_sizes_bytes,
            mfp_align,
            stage: RegisterSizes {},
        }
    }

    /// Registers an MFP with the given size at the given index.
    ///
    /// This registration is necessary to start assembling the MEP and writing the MFPs into the right slot.
    ///
    /// `idx` needs to be in 0..num_mfps.
    pub fn register_mfp(&mut self, idx: usize, size_bytes: usize) -> &mut Self {
        let _ = self.mfp_sizes_bytes[idx]
            .replace(NonZero::new(size_bytes).expect("non zero"))
            .is_none_or(|_| panic!("mfp {idx} already registered"));
        self
    }

    /// Marks the registration phase as complete and starts assembling the MEP.
    ///
    /// This allows writing the MFPs into the right slot.
    /// This function consumes the builder and returns a new builder that can be used to write the MFPs.
    pub fn start_assembling(self) -> ZeroCopyMepBuilder<'a, StoreMfps> {
        let num_mfps = self.num_mfps();
        let mut total_size: usize = 0;

        write_offsets(
            self.buffer,
            num_mfps,
            offsets_iter(
                self.mfp_sizes_bytes
                    .iter()
                    .map(|s| s.expect("all mfp sizes are set").into()),
                self.mfp_align,
                &mut total_size,
            ),
        );

        write_const_header(
            self.buffer,
            MultiEventPacketConstHeader {
                magic: MultiEventPacket::MAGIC,
                num_mfps: num_mfps.try_into().expect("number of mfps fits into u16"),
                packet_size: (total_size / size_of::<u32>())
                    .try_into()
                    .expect("packet size fits into u32"),
            },
        );

        ZeroCopyMepBuilder {
            buffer: self.buffer,
            mfp_align: self.mfp_align,
            mfp_sizes_bytes: self.mfp_sizes_bytes,
            stage: StoreMfps { total_size },
        }
    }
}

impl<'a> ZeroCopyMepBuilder<'a, StoreMfps> {
    /// Retunrns the **byte** index range of a MFP slot inside the underlying buffer.
    ///
    /// This may be useful for some RDMA applications.
    pub fn get_mfp_range(&self, index: usize) -> Range<usize> {
        let offset =
            access_offsets(self.buffer, self.num_mfps())[index] as usize * size_of::<u32>();
        let size = self.mfp_sizes_bytes()[index];
        offset..(offset + size)
    }

    /// Returns a mutable byte slice to the `index`ed MFP slot.
    ///
    /// This slice can be used to write the MFP.
    /// Its size equals the size previously specified for this slot.
    pub fn get_mfp_slot(&mut self, index: usize) -> &mut [u8] {
        let range = self.get_mfp_range(index);
        &mut cast_slice_mut(self.buffer)[range]
    }

    /// Returns an iterator over a range of MFP slots.
    pub fn get_mfp_slots(&mut self, indices: Range<usize>) -> impl Iterator<Item = &mut [u8]> {
        let buffer = cast_slice_mut(self.buffer) as *mut [u8];

        // SAEFTY: mfp ranges for different indices don't overlap,
        // get_mfp_range does not touch the mfp part of the buffer (only header)
        indices.map(move |i| &mut unsafe { &mut *buffer }[self.get_mfp_range(i)])
    }

    /// Returns an iterator over multiple arbitrary MFP slots.
    ///
    /// If the required slots form one contigous interval, use the safe [`Self::get_mfp_slots`] instead.
    ///
    /// # Safety
    /// The `indices` iterator **must not** produce duplicate indices.
    pub unsafe fn get_mfp_slots_unsafe(
        &mut self,
        indices: impl Iterator<Item = usize>,
    ) -> impl Iterator<Item = &mut [u8]> {
        let buffer = cast_slice_mut(self.buffer) as *mut [u8];

        // SAEFTY: mfp ranges for different indices don't overlap, indices are different as of precondition.
        // get_mfp_range does not touch the mfp part of the buffer (only header)
        indices.map(move |i| &mut unsafe { &mut *buffer }[self.get_mfp_range(i)])
    }

    /// Convenience method for trying to cast the data in some slot to a MFP.
    pub fn get_mfp(&self, index: usize) -> Result<&MultiFragmentPacket, FromRawBytesError> {
        let data = &cast_slice::<_, u8>(self.buffer)[self.get_mfp_range(index)];
        MultiFragmentPacket::from_raw_bytes(data)
    }

    /// This method completes building the MEP.
    ///
    /// You need to insure that all MFPs have the same event id and number of fragments to produce a valid MEP.
    pub fn finish(self) -> Result<&'a MultiEventPacket, FromRawBytesError> {
        let num_mfps = self.num_mfps();
        let header_size_u32 = total_header_size(num_mfps) / size_of::<u32>();
        let (header, rest) = self.buffer.split_at_mut(header_size_u32);

        write_source_ids(
            header,
            num_mfps,
            offsets_iter(
                self.mfp_sizes_bytes.iter().copied().map(bytemuck::cast),
                self.mfp_align,
                &mut 0,
            )
            .map(|offset_bytes| {
                MultiFragmentPacket::from_raw_bytes(bytemuck::cast_slice::<_, u8>(
                    &rest[(offset_bytes / size_of::<u32>() - header_size_u32)..],
                ))
                .expect("valid mfp")
                .source_id()
            }),
        );

        let mep_slice = &self.buffer[0..self.stage.total_size / size_of::<u32>()];
        MultiEventPacket::from_raw_bytes(mep_slice)
    }

    fn mfp_sizes_bytes(&self) -> &[usize] {
        cast_slice(self.mfp_sizes_bytes)
    }
}

#[cfg(test)]
mod test {
    use std::ptr::slice_from_raw_parts_mut;

    use ebutils::{FragmentType, SourceId};
    use multi_fragment_packet::MultiFragmentPacketOwned;

    use crate::zerocopy_builder::ZeroCopyMepBuilder;

    #[test]
    fn test() {
        let mut buffer = vec![0u32; 1 << 10];
        let mut cache = [0; 2];

        for _ in 0..1 {
            let mut builder = ZeroCopyMepBuilder::new(&mut buffer, &mut cache, 4);

            let mfp0 = MultiFragmentPacketOwned::builder()
                .with_event_id(11)
                .with_source_id(SourceId::new_odin(0))
                .with_align_log(2)
                .with_fragment_version(1)
                .add_fragments([(FragmentType::ODIN, ebutils::odin::dummy_odin_payload(11))])
                .build();
            let mfp1 = MultiFragmentPacketOwned::builder()
                .with_event_id(11)
                .with_source_id(SourceId::new(ebutils::SubDetector::UtA, 456))
                .with_align_log(2)
                .with_fragment_version(1)
                .add_fragments([(FragmentType::DAQ, b"hello world!")])
                .build();
            builder.register_mfp(1, mfp1.packet_size() as _);
            builder.register_mfp(0, mfp0.packet_size() as _);
            let mut builder = builder.start_assembling();
            let memory = builder.get_buffer_range();
            let memory: &mut [u8] = bytemuck::cast_slice_mut(unsafe {
                &mut *slice_from_raw_parts_mut(
                    memory.start,
                    memory.end.offset_from_unsigned(memory.start),
                )
            });

            memory[builder.get_mfp_range(1)].copy_from_slice(mfp1.raw_packet_data());
            memory[builder.get_mfp_range(0)].copy_from_slice(mfp0.raw_packet_data());

            let mep = builder.finish().expect("valid mep");

            assert_eq!(mep.num_mfps(), 2);
            assert!(mep.get_mfp(0).unwrap().source_id().is_odin());
            assert_eq!(
                mep.get_mfp(1).unwrap().fragment(0).unwrap().payload_bytes(),
                mfp1.fragment(0).unwrap().payload_bytes()
            );
        }
    }
}
