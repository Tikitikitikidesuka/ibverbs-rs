use std::{fmt::Debug, mem::offset_of};

use bytemuck::{Pod, Zeroable, bytes_of};

#[repr(C, packed(4))]
#[derive(Copy, Clone, Zeroable)]
pub struct MdfHeader<H = Unknown>
where
    H: SpecificHeaderType,
{
    /// in units of ~~u32~~ bytes!!! contrary to the specification
    pub(crate) lengths: [u32; 3],
    pub(crate) checksum: u32,
    pub(crate) compression: u8,
    pub(crate) header_type_and_size: SpecificHeaderTypeAndSize,
    pub(crate) data_type: u8,
    pub(crate) _spare: u8,
    pub(crate) specific_header: H,
}

impl<H: SpecificHeaderType> MdfHeader<H> {
    pub const HEADER_SIZE_MIN_U32: usize = size_of::<MdfHeader<Unknown>>() / size_of::<u32>();
    pub fn as_bytes(&self) -> &[u8] {
        bytes_of(self)
    }

    /// Checks the length fields in the header and returns Some if valid.
    ///
    /// For now, this checks equality of all of them and does not take a majority decision yet.
    pub fn length_bytes(self) -> Option<u32> {
        let mut lengths_rot = self.lengths;
        lengths_rot.rotate_left(1);
        (lengths_rot == self.lengths).then_some(self.lengths[0])
    }

    pub fn length_u32(self) -> Option<usize> {
        self.length_bytes().map(|l| l as usize / size_of::<u32>())
    }

    /// Just returns the first length field, without checking for equality.
    /// Be careful when using this method, the other two lengths could be the correct ones.
    pub fn length_unchecked(self) -> u32 {
        self.lengths[0]
    }
}

/// ## Safety
/// Specific header has size multiple of 4.
/// All other requirements are satisfied and checkable by the derive(Pod) Macro
unsafe impl<H: SpecificHeaderType> Pod for MdfHeader<H> {}

impl<H: SpecificHeaderType> Debug for MdfHeader<H> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = f.debug_struct("MdfHeader");
        let debug = debug
            .field("lengths", &self.lengths)
            .field("checksum", &self.checksum)
            .field("compression", &self.compression)
            .field("header_type_and_size", &self.header_type_and_size)
            .field("data_type", &self.data_type)
            .field("_spare", &self._spare);
        let specific_start = unsafe {
            (&self as *const _ as *const u32).byte_add(offset_of!(Self, specific_header))
        };
        let debug = match self.header_type_and_size.header_type() {
            SingleEvent::HEADER_TYPE => {
                let specific = unsafe { &*(specific_start as *const SingleEvent) };

                debug.field("specific_header", specific)
            }
            MultiPurpose::HEADER_TYPE => {
                let multi = unsafe { &*(specific_start as *const MultiPurpose) };
                debug.field("specific_header", multi)
            }
            _ => {
                let specific = self.specific_header;
                debug.field("specific_header", &specific)
            }
        };

        debug.finish()
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct SpecificHeaderTypeAndSize(u8);
impl SpecificHeaderTypeAndSize {
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
impl Debug for SpecificHeaderTypeAndSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HeaderTypeAndSize")
            .field("type", &self.header_type())
            .field("size_u32", &self.size_u32())
            .finish()
    }
}

impl MdfHeader<SingleEvent> {
    pub fn new_simple(payload_size: usize) -> Self {
        let length_32 =
            u32::try_from(payload_size + size_of::<Self>()).expect("payload size fits in u32");

        MdfHeader {
            lengths: [length_32; 3],
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
/// ## Safety
/// Size of type needs to be multiple of 4.
pub unsafe trait SpecificHeaderType: internal::Sealed + Copy + Pod + Debug {
    const HEADER_TYPE: u8;
    fn header_type_and_size() -> SpecificHeaderTypeAndSize;
}

#[repr(C, packed(4))]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
/// A type of MDF record that contains multiple fragments for a single event.
///
/// It must contain exactly one odin fragment.
pub struct SingleEvent {
    pub event_mask: u128,
    pub run_number: u32,
    pub orbit_count: u32,
    pub bunch_identifier: u32,
}
impl internal::Sealed for SingleEvent {}
/// ## Safety
/// Size is 28, multiple of 4.
unsafe impl SpecificHeaderType for SingleEvent {
    const HEADER_TYPE: u8 = 3;

    fn header_type_and_size() -> SpecificHeaderTypeAndSize {
        SpecificHeaderTypeAndSize::from_type_and_size(Self::HEADER_TYPE, Self::HEADER_SIZE_U32)
    }
}
impl SingleEvent {
    pub const HEADER_SIZE_U32: u8 = 7;
}

#[derive(Clone, Copy, Pod, Zeroable, Debug)]
#[repr(C, align(4))]
pub struct MultiPurpose {}
impl internal::Sealed for MultiPurpose {}
/// ## Safety
/// Size is 0, multiple of 4.
unsafe impl SpecificHeaderType for MultiPurpose {
    const HEADER_TYPE: u8 = 4;
    fn header_type_and_size() -> SpecificHeaderTypeAndSize {
        SpecificHeaderTypeAndSize::from_type_and_size(Self::HEADER_TYPE, 0)
    }
}

pub mod multi_purpose {
    use strum::FromRepr;

    #[repr(u8)]
    #[derive(Copy, Clone, FromRepr)]
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

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct Unknown;
impl internal::Sealed for Unknown {}
/// ## Safety
/// Size is 0, multiple of 4.
unsafe impl SpecificHeaderType for Unknown {
    const HEADER_TYPE: u8 = 0;

    fn header_type_and_size() -> SpecificHeaderTypeAndSize {
        unimplemented!()
    }
}

impl Debug for Unknown {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Unknown").finish_non_exhaustive()
    }
}
