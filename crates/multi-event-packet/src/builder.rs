use std::{borrow::Cow, slice};

use bytemuck::cast_slice_mut;
use ebutils::{fragment_type::FragmentType, source_id::SourceId};
use multi_fragment_packet::{MultiFragmentPacket, MultiFragmentPacketOwned};
use thiserror::Error;
use tracing::instrument;

use crate::{
    MultiEventPacket, MultiEventPacketConstHeader, MultiEventPacketOwned, Offset, header_size,
    src_ids_size,
};

/// This is a builder struct for constructing an MEP out of MFPs for the same events and different source ids.
///
/// At least one MFP from an ODIN source is required ([`SourceId::is_odin`] and Containing [`ebutils::OdinPayload`] fragments).
///
/// # Example
/// ```
/// # use multi_event_packet::MultiEventPacketBuilder;
/// # use ebutils::{odin::dummy_odin_payload, FragmentType, SourceId};
/// # use multi_fragment_packet::MultiFragmentPacketOwned;
/// # let mfp1 = MultiFragmentPacketOwned::builder().with_event_id(0).with_source_id(SourceId(0)).with_align_log(2).with_fragment_version(0)
/// # .add_fragment(FragmentType::Odin, dummy_odin_payload(0)).build();
/// # let mfp2 = MultiFragmentPacketOwned::builder().with_event_id(0).with_source_id(SourceId(12213)).with_align_log(2).with_fragment_version(0)
/// # .add_fragment(FragmentType::DAQ, b"Hello").build();
/// // getting mfp1 and mfp2 from somewhere
/// let mep = MultiEventPacketBuilder::with_capacity(2)
///     .add_mfp(mfp1).unwrap()
///     .add_mfp(mfp2).unwrap()
///     .build().unwrap();
/// ```
#[derive(Default)]
pub struct MultiEventPacketBuilder<'a> {
    mfps: Vec<Cow<'a, MultiFragmentPacket>>,
    mfp_align: Option<usize>,
    odin_added: bool,
    allow_superfluous_odin_fragments: bool,
}

impl<'a> MultiEventPacketBuilder<'a> {
    pub const DEFAULT_MFP_ALIGN: usize = align_of::<u64>();

    /// Creates a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new builder with a preallocated capacity for `capacity` MFP references.
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
    pub fn check_mfp_compatibility(&self, test_mfp: &MultiFragmentPacket) -> Result<()> {
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

        if self.odin_added
            && test_mfp.source_id().is_odin()
            && !self.allow_superfluous_odin_fragments
        {
            return Err(EventBuilderError::SuperfluousOdinFragment);
        }
        Ok(())
    }

    /// Allows to add more than one odin fragment.
    ///
    /// Generally, this is unwanted and disabled but may be useful for testing purposes when only odin fragments are created.
    pub fn allow_superfluous_odin_fragments(&mut self) {
        self.allow_superfluous_odin_fragments = true;
    }

    /// Adds an MFP to this builder, only requiring a reference to it.
    ///
    /// The MFP needs to cover the same events as previously added MFPs.
    /// For more details on the requirements, see [`Self::check_mfp_compatibility`].
    pub fn add_mfp_ref(&mut self, mfp: &'a MultiFragmentPacket) -> Result<&mut Self> {
        self.check_mfp_compatibility(mfp)?;
        if mfp.source_id().is_odin() {
            self.odin_added = true;
        }
        self.mfps.push(Cow::Borrowed(mfp));
        Ok(self)
    }

    /// Adds an owned MFP to this builder.
    ///
    /// This is useful if you don't want or can keep around all the MFPs while building.
    ///
    /// The MFP needs to cover the same events as previously added MFPs.
    /// For more details on the requirements, see [`Self::check_mfp_compatibility`].
    pub fn add_mfp(&mut self, mfp: MultiFragmentPacketOwned) -> Result<&mut Self> {
        self.check_mfp_compatibility(&mfp)?;
        if mfp.source_id().is_odin() {
            self.odin_added = true;
        }
        self.mfps.push(Cow::Owned(mfp));
        Ok(self)
    }

    /// Sets the alignment the MFPs in the MEP should have.
    ///
    /// This can be set at any time before build.
    /// The default value is [`Self::DEFAULT_MFP_ALIGN`].
    pub fn set_mfp_align(&mut self, align: usize) -> &mut Self {
        self.mfp_align = Some(align);
        self
    }

    /// Builds an MEP from the provided MFPs.
    ///
    /// Resets the builder afterwards, so it can be reused without reallocating the internal buffer.
    /// # Errors
    /// If no ODIN MFP has been added.
    ///
    /// In case of `Err`, the builder is not reset!
    #[instrument(skip(self))]
    pub fn build(&mut self) -> Result<MultiEventPacketOwned> {
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
            header.magic = MultiEventPacket::MAGIC;
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
                assert!(mfp.fragment_iter().all(|f| {
                    f.fragment_type_parsed()
                        .is_some_and(|t| t == FragmentType::Odin)
                }));
            }
            let data = cast_slice_mut(data.as_mut());
            let data = &mut data[offset..];
            let data = &mut data[..mfp.packet_size() as usize];
            data.copy_from_slice(mfp.raw_packet_data());
        }

        self.reset_mfps();

        Ok(unsafe { MultiEventPacketOwned::from_data(data) })
    }

    /// Resets the builder for reuse.
    ///
    /// Clears the internal buffer, removing all mfps, but not the alignment. Does not deallocate.
    /// This is useful if you want to avoid any allocations while building MEPs.
    pub fn reset_mfps(&mut self) {
        self.mfps.clear();
    }

    /// Generates the MFP offsets in bytes from the start of the header.
    /// Also stores the total size in the out parameter.
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

/// Errors that can occur when building MEPs.
#[derive(Debug, Error)]
pub enum EventBuilderError {
    /// You tried to build an MEP with different event ids.
    #[error(
        "Trying to add a mfp with different event ID ({got}) than previously added ({expected})."
    )]
    MismatchingEventId { expected: u64, got: u64 },
    /// You tried to build an MEP with differently sized MFPs.
    #[error(
        "Trying ot add an mfp with different number of fragments ({got}) than previously added ({expected})."
    )]
    MismatchingFragmentCount { expected: u16, got: u16 },
    /// You tried to add more than one ODIN MFP.
    #[error("An odin MFP was already added (Sub detector 0), you tried to add another one.")]
    SuperfluousOdinFragment,
    /// You tried to build an MFP without an odin fragment.
    #[error("No odin MFP was added. Exactly one Odin MFP is required.")]
    NoOdinFragment,
}

/// A convenience type definition for a [`Result`] with its error defaulting to [`EventBuilderError`].
pub type Result<T, E = EventBuilderError> = std::result::Result<T, E>;

#[cfg(test)]
mod test {
    use ebutils::{
        fragment_type::FragmentType,
        odin::dummy_odin_payload,
        source_id::{SourceId, SubDetector},
    };
    use multi_fragment_packet::{MultiFragmentPacket, MultiFragmentPacketBuilder};

    use crate::{MultiEventPacket, MultiEventPacketOwned};

    #[test]
    fn test_build_mep() {
        let u64_align = align_of::<u64>().ilog2().try_into().unwrap();
        let mfp = MultiFragmentPacketBuilder::new()
            .with_event_id(123456)
            .with_align_log(u64_align)
            .with_fragment_version(22)
            .with_magic(MultiFragmentPacket::VALID_MAGIC)
            .with_source_id(SourceId::new(SubDetector::Odin, 0))
            .add_fragment(FragmentType::Odin, dummy_odin_payload(123456))
            .add_fragment(FragmentType::Odin, dummy_odin_payload(123457))
            .build();
        let mfp2 = MultiFragmentPacketBuilder::new()
            .with_event_id(123456)
            .with_align_log(u64_align)
            .with_fragment_version(25)
            .with_magic(MultiFragmentPacket::VALID_MAGIC)
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
            .with_magic(MultiFragmentPacket::VALID_MAGIC)
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

        let mut mep = MultiEventPacketOwned::builder();
        mep.add_mfp(mfp).unwrap();
        mep.add_mfp_ref(&mfp2).err().unwrap(); // expect errors as wrong num fragments
        mep.add_mfp_ref(&mfp3).unwrap();
        mep.add_mfp_ref(&mfp3).unwrap(); // expect errors as wrong num fragments
        let mep = mep.build().unwrap();

        assert_eq!(mep.magic(), MultiEventPacket::MAGIC);
        assert_eq!(mep.num_mfps(), 3);
        assert_eq!(mep.packet_size_u32(), 99);
        assert_eq!(
            mep.mfp_source_ids(),
            &[SourceId(0), SourceId(8247), SourceId(8247)]
        );
        assert_eq!(mep.mfp_offsets_u32(), &[7, 35, 67]);
        println!("{mep:?}");
        println!("size: {}", size_of_val(mep.data()) / size_of::<u32>());

        assert_eq!(3, mep.mfp_iter().len());
        assert_eq!(0, mep.mfp_iter_srcid_range(SourceId(1)..SourceId(10)).len());
        println!("{:?}", mep.get_mfp(0).unwrap().fragment(0));
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
            assert_eq!(fp.magic(), MultiFragmentPacket::VALID_MAGIC);
        }
    }

    #[test]
    fn test_no_odin() {
        MultiEventPacketOwned::builder().build().unwrap_err();
    }
}
