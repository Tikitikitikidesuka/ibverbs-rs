use std::{marker::PhantomData, mem, ptr::slice_from_raw_parts, slice};

use crate::multi_purpose::MultiPurposeType;

#[repr(C)]
pub struct MdfGenericHeader {
    length_1: u32,
    length_2: u32,
    length_3: u32,
    checksum: u32,
    _spare: u8,
    data_type: u8,
    header_type: u8,
    compression: u8,
}

mod internal {
    pub(crate) trait Sealed {}
}

#[allow(private_bounds)]
pub trait SpecificHeaderType: internal::Sealed {}

#[repr(C, packed(4))]
pub struct SingleEvent {
    event_mask: u128,
    run_number: u32,
    orbit_count: u32,
    bunch_identifier: u32,
}
impl internal::Sealed for SingleEvent {}
impl SpecificHeaderType for SingleEvent {}

pub struct Unknown {}
impl internal::Sealed for Unknown {}
impl SpecificHeaderType for Unknown {}

pub struct MultiPurpose {}
impl internal::Sealed for MultiPurpose {}
impl SpecificHeaderType for MultiPurpose {}

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

pub struct MdfRecordRef<H: SpecificHeaderType, R> {
    generic_header: MdfGenericHeader,
    specific_header: H,
    _content_type: PhantomData<R>,
}

impl<H: SpecificHeaderType, R> MdfRecordRef<H, R> {
    pub const SINGLE_EVENT_HEADER_VERSION: u8 = 3;
    pub const SINGLE_EVENT_HEADER_SIZE_U32: u8 = 7;
    pub const SINGLE_EVENT_HEADER_SIZE_BYTES: usize =
        Self::SINGLE_EVENT_HEADER_SIZE_U32 as usize * size_of::<u32>();

    /// Returns the entire record length in units of `u32`.
    pub fn size_u32(&self) -> u32 {
        assert_eq!(self.generic_header.length_1, self.generic_header.length_2);
        assert_eq!(self.generic_header.length_2, self.generic_header.length_3);
        self.generic_header.length_1
    }

    /// Returns the entire record length in bytes.
    pub fn size_bytes(&self) -> usize {
        self.size_u32() as usize * size_of::<u8>()
    }

    pub fn specific_header_type(&self) -> u8 {
        // todo is this the right way around?
        self.generic_header.header_type >> 4
    }

    pub fn specific_header_size_bytes(&self) -> usize {
        // todo is this the right way around?
        (self.generic_header.header_type & 0xF) as usize * size_of::<u32>()
    }

    pub fn specific_header(&self) -> &H {
        &self.specific_header
    }

    pub fn specific_header_raw(&self) -> &[u32] {
        unsafe {
            slice::from_raw_parts(
                (self as *const Self as *const u32).byte_add(size_of_val(&self.generic_header)),
                self.specific_header_size_bytes(),
            )
        }
    }

    pub fn body(&self) -> &[u8] {
        let offset = size_of_val(&self.generic_header) + self.specific_header_size_bytes();
        unsafe {
            slice::from_raw_parts(
                (self as *const Self as *const u8).byte_add(offset),
                self.size_bytes() - offset,
            )
        }
    }
}

impl<R> MdfRecordRef<MultiPurpose, R> {
    pub fn get_multi_purpose_type(&self) -> MultiPurposeType {
        // todo move assert to constructor
        assert!(
            [
                MultiPurposeType::BodyTypeBanks.value(),
                MultiPurposeType::BodyTypeMEP.value()
            ]
            .contains(&self.generic_header.data_type),
        );

        unsafe { mem::transmute(self.generic_header.data_type) }
    }
}
