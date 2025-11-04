use std::{mem, slice};

use bytemuck::{NoUninit, bytes_of};

use crate::{fragment::MdfFragmentRef, multi_purpose::MultiPurposeType};
pub mod builder;
pub mod fragment;
// mod transpose;

#[cfg(not(target_endian = "little"))]
compile_error!("Only little endian supported!");

#[repr(C, packed(4))]
#[derive(Copy, Clone)]
pub struct MdfHeader<H: SpecificHeaderType> {
    /// in units of u32
    length_1: u32,
    length_2: u32,
    length_3: u32,
    checksum: u32,
    compression: u8,
    header_type: u8,
    data_type: u8,
    _spare: u8,
    specific_header: H,
}

/// SAFETY: no padding, also not for generic field `H`.
unsafe impl<H: SpecificHeaderType> NoUninit for MdfHeader<H> {}

pub fn header_type(header_type: u8, header_size_u32: u8) -> u8 {
    assert!(header_size_u32 < 0xF);
    assert!(header_type < 0xF);
    header_type << 4 | header_size_u32
}

impl<H: SpecificHeaderType> MdfHeader<H> {
    pub fn as_byets(&self) -> &[u8] {
        bytes_of(self)
    }
}

impl MdfHeader<SingleEvent> {
    pub fn new(payload_size: usize) -> Self {
        let length_32 = (payload_size + size_of::<Self>()).div_ceil(size_of::<u32>()) as u32;
        MdfHeader {
            length_1: length_32,
            length_2: length_32,
            length_3: length_32,
            checksum: 0,
            compression: 0,
            header_type: SingleEvent::header_type(),
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
pub trait SpecificHeaderType: internal::Sealed + Copy + NoUninit {
    const HEADER_TYPE: u8;
    fn header_type() -> u8;
}

#[repr(C, packed(4))]
#[derive(Clone, Copy, NoUninit)]
pub struct SingleEvent {
    pub event_mask: u128,
    pub run_number: u32,
    pub orbit_count: u32,
    pub bunch_identifier: u32,
}
impl internal::Sealed for SingleEvent {}
impl SpecificHeaderType for SingleEvent {
    const HEADER_TYPE: u8 = 3;

    fn header_type() -> u8 {
        header_type(Self::HEADER_TYPE, Self::HEADER_SIZE_U32)
    }
}
impl SingleEvent {
    pub const HEADER_SIZE_U32: u8 = 7;
    // pub const SINGLE_EVENT_HEADER_SIZE_BYTES: usize =
    // Self::SINGLE_EVENT_HEADER_SIZE_U32 as usize * size_of::<u32>();
}

// pub struct Unknown<const S: u8> {}
// impl<const S: u8> internal::Sealed for Unknown<S> {}
// impl<const S: u8> SpecificHeaderType for Unknown<S> {
//     const HEADER_TYPE: u8 = S;
// }

#[derive(Clone, Copy, NoUninit)]
#[repr(C)]
pub struct MultiPurpose {}
impl internal::Sealed for MultiPurpose {}
impl SpecificHeaderType for MultiPurpose {
    const HEADER_TYPE: u8 = 4;
    fn header_type() -> u8 {
        header_type(Self::HEADER_TYPE, 0)
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

pub struct MdfRecordRef<H: SpecificHeaderType> {
    generic_header: MdfHeader<H>,
}

impl<H: SpecificHeaderType> MdfRecordRef<H> {
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

    pub fn specific_header(&self) -> H {
        self.generic_header.specific_header
    }

    pub fn specific_header_raw(&self) -> &[u32] {
        unsafe {
            slice::from_raw_parts(
                (self as *const Self as *const u32).byte_add(size_of_val(&self.generic_header)),
                self.specific_header_size_bytes() / size_of::<u32>(),
            )
        }
    }

    pub fn body_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(self.body_u32())
    }

    pub fn body_u32(&self) -> &[u32] {
        let offset = size_of_val(&self.generic_header) + self.specific_header_size_bytes();
        assert!(offset.is_multiple_of(size_of::<u32>()));
        let offset32 = offset / size_of::<u32>();

        unsafe {
            slice::from_raw_parts(
                (self as *const Self as *const u32).add(offset32),
                self.size_u32() as usize - offset32,
            )
        }
    }
}

impl MdfRecordRef<SingleEvent> {
    pub fn fragments(&self) -> impl Iterator<Item = &MdfFragmentRef> {
        MdfFragmentIterator {
            remaining_data: self.body_u32(),
        }
    }
}

pub struct MdfFragmentIterator<'a> {
    remaining_data: &'a [u32],
}

impl<'a> Iterator for MdfFragmentIterator<'a> {
    type Item = &'a MdfFragmentRef;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.remaining_data.is_empty() {
            let ret = unsafe { MdfFragmentRef::from_raw(self.remaining_data) };

            let frag_size_32 = ret.size_bytes().div_ceil(size_of::<u32>());

            self.remaining_data = &self.remaining_data[frag_size_32..];

            Some(ret)
        } else {
            None
        }
    }
}

impl MdfRecordRef<MultiPurpose> {
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
