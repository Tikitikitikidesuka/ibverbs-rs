use core::fmt;
use std::{fmt::Debug, mem, slice};

use bytemuck::cast_slice_mut;
use thiserror::Error;

use crate::{
    fragment::MdfFragmentRef,
    header::{
        MdfHeader, MultiPurpose, SingleEvent, SpecificHeaderType, Unknown,
        multi_purpose::MultiPurposeType,
    },
};
pub mod fragment;
pub mod header;
pub mod writer;

pub use writer::WriteMdf;

#[cfg(not(target_endian = "little"))]
compile_error!("Only little endian supported!");

pub struct MdfRecordRef<H: SpecificHeaderType> {
    generic_header: MdfHeader<H>,
}

impl<H: SpecificHeaderType> MdfRecordRef<H> {
    /// Returns the entire record length in units of `u32`.
    pub fn size_u32(&self) -> u32 {
        assert_eq!(
            self.generic_header.length_1, self.generic_header.length_2,
            "{:?}",
            self.generic_header
        );
        assert_eq!(
            self.generic_header.length_2, self.generic_header.length_3,
            "{:?}",
            self.generic_header
        );
        self.generic_header.length_1
    }

    /// Returns the entire record length in bytes.
    pub fn size_bytes(&self) -> usize {
        self.size_u32() as usize * size_of::<u32>()
    }

    pub fn specific_header_type(&self) -> u8 {
        // todo is this the right way around?
        self.generic_header.header_type_and_size.header_type()
    }

    pub fn specific_header_size_bytes(&self) -> usize {
        // todo is this the right way around?
        self.generic_header.header_type_and_size.size_bytes()
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
        // unknown has zero size specific header, account for separately
        let offset = size_of::<MdfHeader<Unknown>>() + self.specific_header_size_bytes();
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

impl MdfRecordRef<Unknown> {
    pub fn try_into_single_event(&self) -> Result<&MdfRecordRef<SingleEvent>, HeaderParseError> {
        self.try_into()
    }
}

impl fmt::Debug for MdfRecordRef<Unknown> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MdfRecordRef")
            .field("generic_header", &self.generic_header)
            .field("body", &self.body_u32())
            .finish()
    }
}

impl fmt::Debug for MdfRecordRef<SingleEvent> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MdfRecordRef")
            .field("generic_header", &self.generic_header)
            .field(
                "fragments",
                &self.fragments().collect::<Vec<_>>().as_slice(),
            )
            .finish()
    }
}

#[derive(Debug, Error)]
pub enum HeaderParseError {
    #[error("Invalid header type: expected {expected} but got {got}")]
    InvalidHeaderType { expected: u8, got: u8 },
    #[error("Invalid header size: expected {expected} but got {got}")]
    InvalidHeaderSize { expected: usize, got: usize },
}

impl<'a> TryFrom<&'a MdfRecordRef<Unknown>> for &'a MdfRecordRef<SingleEvent> {
    type Error = HeaderParseError;

    fn try_from(
        other: &'a MdfRecordRef<Unknown>,
    ) -> Result<&'a MdfRecordRef<SingleEvent>, Self::Error> {
        if other.specific_header_type() != SingleEvent::HEADER_TYPE {
            Err(HeaderParseError::InvalidHeaderType {
                expected: SingleEvent::HEADER_TYPE,
                got: other.specific_header_type(),
            })
        } else if other.specific_header_size_bytes() != size_of::<SingleEvent>() {
            Err(HeaderParseError::InvalidHeaderSize {
                expected: size_of::<SingleEvent>(),
                got: other.specific_header_size_bytes(),
            })
        } else {
            Ok(unsafe { &*(other as *const MdfRecordRef<_> as *const MdfRecordRef<SingleEvent>) })
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
        if self.remaining_data.is_empty() {
            return None;
        }

        let ret = unsafe { MdfFragmentRef::from_raw(self.remaining_data) };

        let frag_size_32 = ret.size_bytes().div_ceil(size_of::<u32>());

        self.remaining_data = &self.remaining_data[frag_size_32..];

        Some(ret)
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

pub struct MdfRecords {
    data: Box<[u32]>,
}

impl Debug for MdfRecords {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.mdf_record_iter()).finish()
    }
}

impl MdfRecords {
    /// # Safety
    /// Data must contain valid mdf records
    pub unsafe fn from_data(data: &[u8]) -> Self {
        let mut boxed = vec![0u32; data.len().div_ceil(size_of::<u32>())].into_boxed_slice();
        cast_slice_mut(&mut boxed)[..data.len()].copy_from_slice(data);
        Self { data: boxed }
    }

    pub fn mdf_record_iter(&self) -> MdfRecordIterator<'_> {
        MdfRecordIterator { data: &self.data }
    }
}

pub struct MdfRecordIterator<'a> {
    data: &'a [u32],
}

impl<'a> Iterator for MdfRecordIterator<'a> {
    type Item = &'a MdfRecordRef<Unknown>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.data.is_empty() {
            return None;
        }

        let record: Self::Item = unsafe { &*self.data.as_ptr().cast::<MdfRecordRef<Unknown>>() };
        self.data = &self.data[record.size_u32() as _..];
        Some(record)
    }
}
