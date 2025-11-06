use std::fmt::Debug;

use bytemuck::{NoUninit, bytes_of};

#[repr(C, packed(4))]
#[derive(Copy, Clone, Debug)]
pub struct MdfHeader<H: SpecificHeaderType> {
    /// in units of u32
    pub(crate) length_1: u32,
    pub(crate) length_2: u32,
    pub(crate) length_3: u32,
    pub(crate) checksum: u32,
    pub(crate) compression: u8,
    pub(crate) header_type_and_size: HeaderTypeAndSize,
    pub(crate) data_type: u8,
    pub(crate) _spare: u8,
    pub(crate) specific_header: H,
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct HeaderTypeAndSize(u8);
impl HeaderTypeAndSize {
    pub fn from_type_and_size(header_type: u8, header_size_u32: u8) -> Self {
        assert!(header_size_u32 < 0xF);
        assert!(header_type < 0xF);
        Self(header_type << 4 | header_size_u32)
    }

    pub fn header_type(self) -> u8 {
        self.0 >> 4
    }

    pub fn size_u32(self) -> u8 {
        self.0 & 0xF
    }
    pub fn size_bytes(self) -> usize {
        self.size_u32() as usize * size_of::<u32>()
    }
}
impl Debug for HeaderTypeAndSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HeaderTypeAndSize")
            .field("type", &self.header_type())
            .field("size_u32", &self.size_u32())
            .finish()
    }
}

/// SAFETY: no padding, also not for generic field `H`.
unsafe impl<H: SpecificHeaderType> NoUninit for MdfHeader<H> {}

impl<H: SpecificHeaderType> MdfHeader<H> {
    pub fn as_bytes(&self) -> &[u8] {
        bytes_of(self)
    }
}

impl MdfHeader<SingleEvent> {
    pub fn new_simple(payload_size: usize) -> Self {
        let length_32 =
            u32::try_from((payload_size + size_of::<Self>()).div_ceil(size_of::<u32>()))
                .expect("payload size fits in u32");
        MdfHeader {
            length_1: length_32,
            length_2: length_32,
            length_3: length_32,
            checksum: 0,
            compression: 0,
            header_type_and_size: SingleEvent::header_type_and_size(),
            data_type: 0,
            _spare: 0,
            specific_header: SingleEvent {
                event_mask: 0,
                // todo for now zero: populate from odin fragment later
                run_number: 0,
                orbit_count: 0,
                bunch_identifier: 0,
            },
        }
    }
}

mod internal {
    pub(crate) trait Sealed {}
}

#[allow(private_bounds)]
pub trait SpecificHeaderType: internal::Sealed + Copy + NoUninit + Debug {
    const HEADER_TYPE: u8;
    fn header_type_and_size() -> HeaderTypeAndSize;
}

#[repr(C, packed(4))]
#[derive(Clone, Copy, NoUninit, Debug)]
pub struct SingleEvent {
    pub event_mask: u128,
    pub run_number: u32,
    pub orbit_count: u32,
    pub bunch_identifier: u32,
}
impl internal::Sealed for SingleEvent {}
impl SpecificHeaderType for SingleEvent {
    const HEADER_TYPE: u8 = 3;

    fn header_type_and_size() -> HeaderTypeAndSize {
        HeaderTypeAndSize::from_type_and_size(Self::HEADER_TYPE, Self::HEADER_SIZE_U32)
    }
}
impl SingleEvent {
    pub const HEADER_SIZE_U32: u8 = 7;
}

#[derive(Clone, Copy, NoUninit, Debug)]
#[repr(C, align(4))]
pub struct MultiPurpose {}
impl internal::Sealed for MultiPurpose {}
impl SpecificHeaderType for MultiPurpose {
    const HEADER_TYPE: u8 = 4;
    fn header_type_and_size() -> HeaderTypeAndSize {
        HeaderTypeAndSize::from_type_and_size(Self::HEADER_TYPE, 0)
    }
}

pub mod multi_purpose {
    #[repr(u8)]
    #[derive(Copy, Clone)]
    pub enum MultiPurposeType {
        /// Sequences of banks produced by TELL1 boards as described in [^1].
        /// [^1]: O.Callot et al., Raw Data Format. EDMS note 565851.
        BodyTypeBanks = 1,
        /// Full MEP records including the transport format as defined in [^3]. This data type is used to process time alignment data [^2].
        /// [^2]: O.Callot, Processing Time-Alignment Events. EDMS note 779819.
        /// [^3]: B.Jost, N.Neufeld, Raw-data transport format. EDMS note 499933
        BodyTypeMEP = 2,
    }

    impl MultiPurposeType {
        pub const fn value(&self) -> u8 {
            *self as u8
        }
    }
}

#[derive(Clone, Copy, NoUninit)]
#[repr(C)]
pub struct Unknown;
impl internal::Sealed for Unknown {}

impl SpecificHeaderType for Unknown {
    const HEADER_TYPE: u8 = 0;

    fn header_type_and_size() -> HeaderTypeAndSize {
        unimplemented!()
    }
}

impl Debug for Unknown {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Unknown").finish_non_exhaustive()
    }
}
