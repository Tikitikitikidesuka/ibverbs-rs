use std::{borrow::Cow, slice};

use multi_fragment_packet::{
    MultiFragmentPacket, MultiFragmentPacketRef, SourceId, fragment_type::FragmentType,
};
use thiserror::Error;
use tracing::instrument;

use crate::{
    MultiEventPacket, MultiEventPacketConstHeader, MultiEventPacketRef, Offset, header_size,
    slice_as_bytes_mut, src_ids_size,
};

#[derive(Debug, Error)]
pub enum EventBuilderError {
    #[error(
        "Trying to add a mfp with different event ID ({got}) than previously added ({expected})."
    )]
    MismatchingEventId { expected: u64, got: u64 },
    #[error(
        "Trying ot add an mfp with different number of fragments ({got}) than previously added ({expected})."
    )]
    MismatchingFragmentCount { expected: u16, got: u16 },
    #[error("An odin MFP was already added (Sub detector 0), you tried to add another one.")]
    SuperfluousOdinFragment,
    #[error("No odin MFP was added. Exactly one Odin MFP is required.")]
    NoOdinFragment,
}

pub type Result<T, E = EventBuilderError> = std::result::Result<T, E>;

#[derive(Default)]
pub struct MultiEventPacketBuilder<'a> {
    mfps: Vec<Cow<'a, MultiFragmentPacketRef>>,
    mfp_align: Option<usize>,
    odin_added: bool,
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

    /// Checks wether the given mfp can be inserted into the same [`MultiEventPacket`]s as the previous, checking its number of fragments and event ids.
    ///
    /// Also checks wether a odin fragment was already added when trying to add another one.
    /// This is checked when adding an mft automatically.
    pub fn check_mfp_event_compatiblity(&self, test_mfp: &MultiFragmentPacketRef) -> Result<()> {
        if let Some(reference_mfp) = self.mfps.first() {
            if test_mfp.event_id() != reference_mfp.event_id() {
                return Err(EventBuilderError::MismatchingEventId {
                    expected: reference_mfp.event_id(),
                    got: test_mfp.event_id(),
                });
            } else if test_mfp.fragment_count() != reference_mfp.fragment_count() {
                return Err(EventBuilderError::MismatchingFragmentCount {
                    expected: reference_mfp.fragment_count(),
                    got: test_mfp.fragment_count(),
                });
            }
        }

        if self.odin_added && test_mfp.source_id().is_odin() {
            return Err(EventBuilderError::SuperfluousOdinFragment);
        }
        Ok(())
    }

    pub fn add_mfp_ref(&mut self, mfp: &'a MultiFragmentPacketRef) -> Result<&mut Self> {
        self.check_mfp_event_compatiblity(mfp)?;
        if mfp.source_id().is_odin() {
            self.odin_added = true;
        }
        self.mfps.push(Cow::Borrowed(mfp));
        Ok(self)
    }

    pub fn add_mfp(&mut self, mfp: MultiFragmentPacket) -> Result<&mut Self> {
        self.check_mfp_event_compatiblity(&mfp)?;
        if mfp.source_id().is_odin() {
            self.odin_added = true;
        }
        self.mfps.push(Cow::Owned(mfp));
        Ok(self)
    }

    pub fn set_mfp_align(&mut self, align: usize) -> &mut Self {
        self.mfp_align = Some(align);
        self
    }

    /// Resets the builder afterwards, so it can be reused without reallocating the internal buffer.
    ///
    /// In case of `Err`, the builder is not reset!
    #[instrument(skip(self))]
    pub fn build(&mut self) -> Result<MultiEventPacket> {
        if !self.odin_added {
            return Err(EventBuilderError::NoOdinFragment);
        }

        self.mfps.sort_by_key(|m| m.source_id());
        let num_mfps = self.mfps.len();
        let num_mfps = u16::try_from(num_mfps).expect("number of mfps does fit into u16");

        // alloc data
        let mut total_size = 0;
        let _ = self.offsets_iter(&mut total_size).count(); // just iterate thorugh to get total size
        let mut data = vec![0u32; total_size / size_of::<u32>()].into_boxed_slice();

        // set header
        {
            let header = unsafe { &mut *(data.as_mut_ptr() as *mut MultiEventPacketConstHeader) };
            header.magic = MultiEventPacketRef::MAGIC;
            header.num_mfps = num_mfps;
            header.packet_size = (total_size / size_of::<u32>())
                .try_into()
                .expect("packet size fits into u32");
        }

        // set src ids
        {
            let src_ids = unsafe {
                data.as_mut_ptr()
                    .byte_add(size_of::<MultiEventPacketConstHeader>())
                    as *mut SourceId
            };
            let src_ids = unsafe { slice::from_raw_parts_mut(src_ids, num_mfps as _) };
            for (src_id, mfp) in src_ids.iter_mut().zip(self.mfps.iter()) {
                *src_id = mfp.source_id();
            }
        }

        // set offsets
        {
            let offset_slots = unsafe {
                data.as_mut_ptr()
                    .byte_add(size_of::<MultiEventPacketConstHeader>())
                    .byte_add(src_ids_size(num_mfps as _)) as *mut Offset
            };
            let offset_slots = unsafe { slice::from_raw_parts_mut(offset_slots, num_mfps as _) };
            for (offset_slot, offset_value) in
                offset_slots.iter_mut().zip(self.offsets_iter(&mut 0))
            {
                *offset_slot = (offset_value / size_of::<u32>())
                    .try_into()
                    .expect("offsets fit into u32");
            }
        }

        // set mfps
        for (offset, mfp) in self.offsets_iter(&mut 0).zip(&self.mfps) {
            if mfp.source_id().is_odin() {
                assert!(mfp.iter().all(|f| {
                    f.fragment_type_parsed()
                        .is_some_and(|t| t == FragmentType::Odin)
                }));
            }
            let data = slice_as_bytes_mut(data.as_mut());
            let data = &mut data[offset..];
            let data = &mut data[..mfp.packet_size() as usize];
            data.copy_from_slice(mfp.raw_packet_data());
        }

        self.reset_mfps();

        Ok(MultiEventPacket { data })
    }

    /// Clears the internal buffer, removing all mfps, but not the alignment. Does not deallocate
    pub fn reset_mfps(&mut self) {
        self.mfps.clear();
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
    use multi_fragment_packet::{
        MultiFragmentPacketBuilder, MultiFragmentPacketRef, SourceId, fragment_type::FragmentType,
        source_id::SubDetector,
    };

    use crate::{MultiEventPacket, MultiEventPacketRef};

    #[test]
    fn test_build_mep() {
        let u64_align = align_of::<u64>().ilog2().try_into().unwrap();
        let mfp = MultiFragmentPacketBuilder::new()
            .with_event_id(123456)
            .with_align_log(u64_align)
            .with_fragment_version(22)
            .with_magic(MultiFragmentPacketRef::VALID_MAGIC)
            .with_source_id(SourceId::new(SubDetector::Odin, 0))
            .add_fragment(
                FragmentType::Odin,
                b"Hello, I am some data. I am trapped here, please free me!",
            )
            .add_fragment(
                FragmentType::Odin,
                b"I do not exist, here is nothing to see!!!",
            )
            .build();
        let mfp2 = MultiFragmentPacketBuilder::new()
            .with_event_id(123456)
            .with_align_log(u64_align)
            .with_fragment_version(25)
            .with_magic(MultiFragmentPacketRef::VALID_MAGIC)
            .with_source_id(SourceId::new(SubDetector::MuonA, 21))
            .add_fragment(
                FragmentType::DAQ,
                b"rsthoeiasrmtarinstitnarsatrnsteinarsietnaein",
            )
            .build();
        let mfp3 = MultiFragmentPacketBuilder::new()
            .with_event_id(123456)
            .with_align_log(u64_align)
            .with_fragment_version(25)
            .with_magic(MultiFragmentPacketRef::VALID_MAGIC)
            .with_source_id(SourceId::new(SubDetector::Rich1, 55))
            .add_fragment(
                FragmentType::DAQ,
                b"rsthoeiasrmtarinstitnarsatrnsteinarsietnaein",
            )
            .add_fragment(
                FragmentType::HcalE,
                b"rsthoeiasrmtarinstitnarsatrnsteinarsietnaein",
            )
            .build();

        let mut mep = MultiEventPacket::builder();
        mep.add_mfp(mfp).unwrap();
        mep.add_mfp_ref(&mfp2).err().unwrap(); // expect errors as wrong num fragments
        mep.add_mfp_ref(&mfp3).unwrap();
        mep.add_mfp_ref(&mfp3).unwrap(); // expect errors as wrong num fragments
        let mep = mep.build().unwrap();

        assert_eq!(mep.magic(), MultiEventPacketRef::MAGIC);
        assert_eq!(mep.num_mfps(), 3);
        assert_eq!(mep.packet_size_u32(), 107);
        assert_eq!(
            mep.mfp_source_ids(),
            &[SourceId(0), SourceId(8247), SourceId(8247)]
        );
        assert_eq!(mep.mfp_offsets_u32(), &[7, 43, 75]);
        println!("{mep:?}");
        println!("size: {}", size_of_val(mep.data()) / size_of::<u32>());

        assert_eq!(3, mep.mfp_iter().len());
        assert_eq!(0, mep.mfp_iter_srcid_range(SourceId(1)..SourceId(10)).len());
        assert_eq!(
            0,
            mep.mfp_iter_srcid_range(SourceId(55555)..SourceId(55555))
                .len()
        );
        assert_eq!(
            2,
            mep.mfp_iter_srcid_range(SubDetector::Rich1.source_id_range())
                .len()
        );
        assert_eq!(
            3,
            mep.mfp_iter_srcid_range(SourceId(0)..SourceId(55556)).len()
        );
        for fp in mep.mfp_iter() {
            println!("{fp:?}");
            assert_eq!(fp.magic(), MultiFragmentPacketRef::VALID_MAGIC);
        }
    }

    #[test]
    fn test_no_odin() {
        MultiEventPacket::builder().build().unwrap_err();
    }
}
