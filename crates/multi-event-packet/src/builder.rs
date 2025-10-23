use std::{borrow::Cow, ops::Add, slice};

use multi_fragment_packet::{MultiFragmentPacket, MultiFragmentPacketRef};

use crate::{MultiEventPacket, MultiEventPacketConstHeader, MultiEventPacketRef};

pub struct MepFomat;

#[derive(Default)]
pub struct MultiEventPacketBuilder<'a> {
    mfps: Vec<Cow<'a, MultiFragmentPacketRef>>,
    format: Option<MepFomat>,
    mfp_align: Option<usize>,
}

impl<'a> MultiEventPacketBuilder<'a> {
    pub const DEFAULT_MFP_ALIGN: usize = size_of::<u64>();

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

        let header_size = size_of::<MultiEventPacketConstHeader>()
            + num_mfps.next_multiple_of(2) * MultiEventPacketRef::SOURCE_ID_SIZE
            + num_mfps * size_of::<u32>();

        let mut data_sum = 0;

        let offsets = self
            .mfps
            .iter()
            .map(|mfp| {
                (mfp.packet_size() as usize)
                    .next_multiple_of(self.mfp_align.unwrap_or(Self::DEFAULT_MFP_ALIGN))
            })
            .scan(&mut data_sum, move |sum, b| {
                let ret = **sum;
                **sum += b;
                Some(ret)
            })
            .collect::<Vec<_>>();
        // todo avoid allocation

        let total_size = header_size + data_sum;

        let mut data = vec![0u8; total_size].into_boxed_slice();

        let header = unsafe { &mut *(data.as_mut_ptr() as *mut MultiEventPacketConstHeader) };
        header.magic = MultiEventPacketRef::MAGIC;
        header.num_mfps = num_mfps as _;
        header.packet_size = (total_size / size_of::<u32>()) as _;
        // set src ids
        let src_ids = unsafe {
            (data.as_mut_ptr() as *mut u16).byte_add(MultiFragmentPacketRef::HEADER_SIZE)
        };
        let src_ids = unsafe { slice::from_raw_parts_mut(src_ids, num_mfps) };
        src_ids
            .iter_mut()
            .zip(self.mfps.iter())
            .for_each(|(src_id, mfp)| *src_id = mfp.source_id());

        // set offsets

        MultiEventPacket { data }
    }
}
