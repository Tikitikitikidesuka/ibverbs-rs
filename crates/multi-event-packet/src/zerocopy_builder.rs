use std::{num::NonZero, ops::Range};

use bytemuck::{cast_slice, checked::cast};
use multi_fragment_packet::{FromRawBytesError, MultiFragmentPacket};

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
pub struct ZeroCopyMepBuilder<'a, S: Stage> {
    buffer: &'a mut [u32],
    mfp_sizes_bytes: &'a mut [Option<NonZero<usize>>],
    mfp_align: usize,
    stage: S,
}

#[allow(private_bounds)]
impl<'a, S: Stage> ZeroCopyMepBuilder<'a, S> {
    pub fn get_buffer_range(&mut self) -> Range<*mut u32> {
        self.buffer.as_mut_ptr_range()
    }

    pub fn num_mfps(&self) -> usize {
        self.mfp_sizes_bytes.len()
    }
}

impl<'a> ZeroCopyMepBuilder<'a, RegisterSizes> {
    /// Length of mfp_size_cache must match the number of MFPs to construct mep for.
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

    /// `idx` needs to be in 0..num_mfps, in correct source id order
    pub fn register_mfp(&mut self, idx: usize, size_bytes: usize) -> &mut Self {
        let _ = self.mfp_sizes_bytes[idx]
            .replace(NonZero::new(size_bytes).expect("non zero"))
            .is_none_or(|_| panic!("mfp {idx} already registered"));
        self
    }

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
    /// Range in **bytes**!
    pub fn get_mfp_range(&self, index: usize) -> Range<usize> {
        let offset =
            access_offsets(self.buffer, self.num_mfps())[index] as usize * size_of::<u32>();
        let size = self.mfp_sizes_bytes()[index];
        offset..(offset + size)
    }

    pub fn get_mfp(&self, index: usize) -> Result<&MultiFragmentPacket, FromRawBytesError> {
        let data = cast_slice(&self.buffer[self.get_mfp_range(index)]);
        println!("{:?}", &data[0..50]);
        MultiFragmentPacket::from_raw_bytes(data)
    }

    /// You need to insure that:
    /// - at least one odin fragment was added, and
    /// - that they are added in the correct order of ascending soruce id,
    /// - all MFPs have the same event id and number of fragments.
    ///
    /// The returned range is in bytes within the buffer.
    pub fn finish(self) -> Range<usize> {
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

        0..self.stage.total_size
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

    use crate::{MultiEventPacket, zerocopy_builder::ZeroCopyMepBuilder};

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

            let range = builder.finish();

            let mep = MultiEventPacket::from_raw_bytes(bytemuck::cast_slice(&memory[range]))
                .expect("valid mep");

            assert_eq!(mep.num_mfps(), 2);
            assert!(mep.get_mfp(0).unwrap().source_id().is_odin());
            assert_eq!(
                mep.get_mfp(1).unwrap().fragment(0).unwrap().payload_bytes(),
                mfp1.fragment(0).unwrap().payload_bytes()
            );
        }
    }
}
