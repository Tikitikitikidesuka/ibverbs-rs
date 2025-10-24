use std::{borrow::Cow, slice};

use multi_fragment_packet::{MultiFragmentPacket, MultiFragmentPacketRef, SourceId};

use crate::{
    MultiEventPacket, MultiEventPacketConstHeader, MultiEventPacketRef, Offset, header_size,
    slice_as_bytes_mut, src_ids_size,
};

#[derive(Default)]
pub struct MultiEventPacketBuilder<'a> {
    mfps: Vec<Cow<'a, MultiFragmentPacketRef>>,
    mfp_align: Option<usize>,
}

impl<'a> MultiEventPacketBuilder<'a> {
    pub const DEFAULT_MFP_ALIGN: usize = align_of::<u64>();

    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            mfps: Vec::with_capacity(capacity),
            ..Self::default()
        }
    }

    pub fn add_mfp_ref(&mut self, mfp: &'a MultiFragmentPacketRef) {
        self.mfps.push(Cow::Borrowed(mfp));
    }

    pub fn add_mfp(&mut self, mfp: MultiFragmentPacket) {
        self.mfps.push(Cow::Owned(mfp));
    }

    pub fn set_mfp_align(&mut self, align: usize) {
        self.mfp_align = Some(align)
    }

    pub fn build(mut self) -> MultiEventPacket {
        self.mfps.sort_by_key(|m| m.source_id());
        let num_mfps = self.mfps.len();

        // alloc data
        let mut total_size = 0;
        let _ = self.offsets_iter(&mut total_size).count(); // just iterate thorugh to get total size
        let mut data = vec![0u32; total_size / size_of::<u32>()].into_boxed_slice();

        // set header
        {
            let header = unsafe { &mut *(data.as_mut_ptr() as *mut MultiEventPacketConstHeader) };
            header.magic = MultiEventPacketRef::MAGIC;
            header.num_mfps = num_mfps as _;
            header.packet_size = (total_size / size_of::<u32>()) as _;
        }

        // set src ids
        {
            let src_ids = unsafe {
                data.as_mut_ptr()
                    .byte_add(size_of::<MultiEventPacketConstHeader>())
                    as *mut SourceId
            };
            let src_ids = unsafe { slice::from_raw_parts_mut(src_ids, num_mfps) };
            for (src_id, mfp) in src_ids.iter_mut().zip(self.mfps.iter()) {
                *src_id = mfp.source_id();
            }
        }

        // set offsets
        {
            let offset_slots = unsafe {
                data.as_mut_ptr()
                    .byte_add(size_of::<MultiEventPacketConstHeader>())
                    .byte_add(src_ids_size(num_mfps)) as *mut Offset
            };
            let offset_slots = unsafe { slice::from_raw_parts_mut(offset_slots, num_mfps) };
            for (offset_slot, offset_value) in
                offset_slots.iter_mut().zip(self.offsets_iter(&mut 0))
            {
                *offset_slot = (offset_value / size_of::<u32>()) as _;
            }
        }

        // set mfps
        for (offset, mfp) in self.offsets_iter(&mut 0).zip(&self.mfps) {
            let data = slice_as_bytes_mut(data.as_mut());
            let data = &mut data[offset..];
            let data = &mut data[..mfp.packet_size() as usize];
            data.copy_from_slice(mfp.raw_packet_data());
        }

        MultiEventPacket { data }
    }

    /// Generates the MFP offsets in bytes from the start of the header.
    /// Also stores the total size in the out parateter.
    fn offsets_iter(&self, total_size: &mut usize) -> impl Iterator<Item = usize> {
        let align = self.mfp_align.unwrap_or(Self::DEFAULT_MFP_ALIGN);
        *total_size = header_size(self.mfps.len());

        self.mfps
            .iter()
            .map(move |mfp| (mfp.packet_size() as usize).next_multiple_of(align))
            .scan(total_size, move |sum, b| {
                let ret = **sum;
                **sum += b;
                Some(ret)
            })
    }
}

#[cfg(test)]
mod test {
    use multi_fragment_packet::{Fragment, MultiFragmentPacketBuilder, MultiFragmentPacketRef};

    use crate::MultiEventPacket;

    #[test]
    fn test_build_mep() {
        let mfp = MultiFragmentPacketBuilder::new()
            .with_event_id(123456)
            .with_align(align_of::<u64>().ilog2() as _)
            .with_fragment_version(22)
            .with_magic(MultiFragmentPacketRef::VALID_MAGIC)
            .with_source_id(55555)
            .lock_header()
            .add_fragment(
                Fragment::new(
                    11,
                    b"Hello, I am some data. I am trapped here, please free me!",
                )
                .unwrap(),
            )
            .add_fragment(Fragment::new(22, b"I do not exist, here is nothing to see!!!").unwrap())
            .build();

        let mut mep = MultiEventPacket::builder();
        mep.add_mfp_ref(&mfp);
        mep.add_mfp_ref(&mfp);
        mep.add_mfp_ref(&mfp);
        let mep = mep.build();
        println!("{mep:?}");
        println!("size: {}", size_of_val(mep.data()) / size_of::<u32>());
    }
}
