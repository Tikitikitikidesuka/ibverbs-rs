use std::{num::NonZero, ops::Range};

use bytemuck::cast_slice;
use multi_fragment_packet::MultiFragmentPacket;

use crate::{
    MultiEventPacket, MultiEventPacketConstHeader,
    builder::{access_offsets, offsets_iter, write_const_header, write_offsets, write_source_ids},
    total_header_size,
    zerocopy_builder::internal::Stage,
};

mod internal {
    pub(super) trait Stage {}
}

pub struct RegisterSizes {}
impl Stage for RegisterSizes {}

pub struct StoreMfps {
    total_size: usize,
}
impl Stage for StoreMfps {}

#[allow(private_bounds)]
pub struct ZeroCopyMepBuilder<S: Stage> {
    buffer: Box<[u32]>,
    mfp_sizes: Box<[Option<NonZero<usize>>]>,
    mfp_align: usize,
    stage: S,
}

#[allow(private_bounds)]
impl<S: Stage> ZeroCopyMepBuilder<S> {
    pub fn get_buffer_range(&mut self) -> Range<*mut u32> {
        self.buffer.as_mut_ptr_range()
    }

    pub fn reset(mut self) -> ZeroCopyMepBuilder<RegisterSizes> {
        self.mfp_sizes.fill(None);

        ZeroCopyMepBuilder {
            stage: RegisterSizes {},
            mfp_sizes: self.mfp_sizes,
            buffer: self.buffer,
            mfp_align: self.mfp_align,
        }
    }

    pub fn num_mfps(&self) -> usize {
        self.mfp_sizes.len()
    }
}

impl ZeroCopyMepBuilder<RegisterSizes> {
    pub fn new(buffer_capacity: usize, num_mfps: usize, mfp_align: usize) -> Self {
        ZeroCopyMepBuilder {
            buffer: vec![0u32; buffer_capacity.div_ceil(size_of::<u32>())].into_boxed_slice(),
            mfp_sizes: vec![None; num_mfps].into_boxed_slice(),
            mfp_align,
            stage: RegisterSizes {},
        }
    }

    /// `idx` needs to be in 0..num_mfps, in correct source id order
    pub fn register_mfp(&mut self, idx: usize, size_u32: NonZero<usize>) -> &mut Self {
        let _ = self.mfp_sizes[idx]
            .replace(size_u32)
            .is_none_or(|_| panic!("mfp {idx} already registered"));
        self
    }

    pub fn start_assembling(mut self) -> ZeroCopyMepBuilder<StoreMfps> {
        let num_mfps = self.num_mfps();
        let mut total_size: usize = 0;

        write_offsets(
            &mut self.buffer,
            num_mfps,
            offsets_iter(
                self.mfp_sizes
                    .iter()
                    .map(|s| s.expect("all mfp sizes are set").into()),
                self.mfp_align,
                &mut total_size,
            ),
        );

        write_const_header(
            &mut self.buffer,
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
            mfp_sizes: self.mfp_sizes,
            stage: StoreMfps { total_size },
        }
    }
}

impl ZeroCopyMepBuilder<StoreMfps> {
    pub fn get_mfp_range(&self, index: usize) -> Range<usize> {
        let offset = access_offsets(&self.buffer, self.num_mfps())[index] as usize;
        offset..(offset + self.mfp_sizes()[index])
    }

    pub fn finish(mut self) -> (Range<usize>, ZeroCopyMepBuilder<RegisterSizes>) {
        let num_mfps = self.num_mfps();
        let header_size = total_header_size(num_mfps);
        let (header, rest) = self.buffer.split_at_mut(header_size);

        write_source_ids(
            header,
            num_mfps,
            offsets_iter(
                self.mfp_sizes.iter().copied().map(bytemuck::cast),
                self.mfp_align,
                &mut 0,
            )
            .map(|offset| {
                MultiFragmentPacket::from_raw_bytes(bytemuck::cast_slice::<_, u8>(
                    &rest[(offset - header_size)..],
                ))
                .expect("valid mfp")
                .source_id()
            }),
        );

        (
            0..self.stage.total_size,
            ZeroCopyMepBuilder {
                buffer: self.buffer,
                mfp_sizes: self.mfp_sizes,
                mfp_align: self.mfp_align,
                stage: RegisterSizes {},
            },
        )
    }

    fn mfp_sizes(&self) -> &[usize] {
        cast_slice(&self.mfp_sizes)
    }
}


