use core::fmt;
use std::{
    fmt::Debug,
    fs::File,
    io::{self, Read},
    os::unix::fs::MetadataExt,
    path::Path,
    slice,
};

use bytemuck::{cast_ref, cast_slice_mut, checked::try_cast_slice};
use std::io::Result as IoResult;
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

#[repr(C, align(4))]
pub struct MdfRecordRef<H: SpecificHeaderType = Unknown> {
    /// Invariant: sizes are valid (i.e. at least two equal).
    generic_header: MdfHeader<H>,
}

impl<H: SpecificHeaderType> MdfRecordRef<H> {
    /// Returns the entire record length in units of `u32`.
    pub fn size_bytes(&self) -> usize {
        self.generic_header.length_bytes().expect("valid") as _
    }

    pub fn size_u32(&self) -> usize {
        self.size_bytes() / size_of::<u32>()
    }

    pub fn specific_header_type(&self) -> u8 {
        self.generic_header.header_type_and_size.header_type()
    }

    pub fn specific_header_size_bytes(&self) -> usize {
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
                self.size_u32() - offset32,
            )
        }
    }
}

impl MdfRecordRef {
    /// Tries to extract an MDF record from the start of the slice, returning the unused rest.
    /// Fails if the slice is too small or contains invalid MDF length information.
    pub fn from_data(data: &[u32]) -> Result<(&Self, &[u32]), MdfFromDataError> {
        let header_data: &[u32; MdfHeader::<Unknown>::HEADER_SIZE_MIN_U32] = &data
            .split_at_checked(MdfHeader::<Unknown>::HEADER_SIZE_MIN_U32)
            .ok_or(MdfFromDataError::TooSmallForHeader(data.len()))?
            .0
            .try_into()
            .expect("size matches");
        let header: &MdfHeader<Unknown> = cast_ref(header_data);

        let Some(length_32) = header.length_u32() else {
            return Err(MdfFromDataError::HeaderLengthMismatch(header.lengths))?;
        };

        if data.len() < length_32 {
            return Err(MdfFromDataError::TotalLengthMismatch {
                expected: length_32 as _,
                got: data.len(),
            });
        }

        let record = unsafe { &*data.as_ptr().cast() };

        Ok((record, &data[length_32..]))
    }

    pub fn try_into_single_event(&self) -> Result<&MdfRecordRef<SingleEvent>, HeaderParseError> {
        if self.specific_header_type() != SingleEvent::HEADER_TYPE {
            Err(HeaderParseError::InvalidHeaderType {
                expected: SingleEvent::HEADER_TYPE,
                got: self.specific_header_type(),
            })
        } else if self.specific_header_size_bytes() != size_of::<SingleEvent>() {
            Err(HeaderParseError::InvalidHeaderSize {
                expected: size_of::<SingleEvent>(),
                got: self.specific_header_size_bytes(),
            })
        } else {
            Ok(unsafe { &*(self as *const MdfRecordRef<_> as *const MdfRecordRef<SingleEvent>) })
        }
    }
}

#[derive(Debug, Error)]
pub enum MdfFromDataError {
    #[error("Slice is to small to even read the header: is {0}, but header is at least {hdr} u32 words", hdr = MdfHeader::<Unknown>::HEADER_SIZE_MIN_U32)]
    TooSmallForHeader(usize),
    #[error("Header length do not match: {0:?}")]
    HeaderLengthMismatch([u32; 3]),
    #[error(
        "Header says record has length {expected}, but the slice you provided only has length {got}."
    )]
    TotalLengthMismatch { expected: usize, got: usize },
}

impl fmt::Debug for MdfRecordRef<Unknown> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MdfRecordRef")
            .field("generic_header", &self.generic_header)
            .field("body", &truncate_data(self.body_u32()))
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
        other.try_into_single_event()
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

        let (frag, rest) = MdfFragmentRef::from_data(self.remaining_data).expect("valid");

        self.remaining_data = rest;

        Some(frag)
    }
}

impl MdfRecordRef<MultiPurpose> {
    pub fn get_multi_purpose_type(&self) -> Option<MultiPurposeType> {
        MultiPurposeType::from_repr(self.generic_header.data_type)
    }
}

pub struct MdfRecords<Store: AsRef<[u32]> = Box<[u32]>> {
    data: Store,
}

impl<Store: AsRef<[u32]>> MdfRecords<Store> {
    pub fn mdf_record_iter(&self) -> MdfRecordIterator<'_> {
        MdfRecordIterator {
            data: self.data.as_ref(),
        }
    }

    pub fn data(&self) -> &[u32] {
        self.data.as_ref()
    }

    pub fn into_inner(self) -> Store {
        self.data
    }
}

impl<'a, Store: AsRef<[u32]>> IntoIterator for &'a MdfRecords<Store> {
    type Item = &'a MdfRecordRef;

    type IntoIter = MdfRecordIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.mdf_record_iter()
    }
}

impl MdfRecords<Box<[u32]>> {
    /// Data must contain valid mdf records.
    /// Data will be copied to ensure alignment.
    pub fn from_data(data: &[u8]) -> Self {
        let mut boxed = vec![0u32; data.len().div_ceil(size_of::<u32>())].into_boxed_slice();
        cast_slice_mut(&mut boxed)[..data.len()].copy_from_slice(data);
        Self { data: boxed }
    }

    /// Reads an MDF file into memory (completely).
    pub fn read_file(file: impl AsRef<Path>) -> IoResult<Self> {
        let mut file: File = File::open(file)?;
        let size = file.metadata()?.size();
        let size = usize::try_from(size).map_err(io::Error::other)?;
        let mut data = vec![0u32; size / size_of::<u32>()].into_boxed_slice();
        file.read_exact(&mut cast_slice_mut(&mut data)[..size])?;
        Ok(Self { data })
    }
}

impl<'a> MdfRecords<&'a [u32]> {
    /// Return `None` if the slice is not 32 bit aligned or has size not multiple of 32 bit.
    pub fn from_aligned_slice(data: &'a [u8]) -> Option<Self> {
        try_cast_slice(data).map(|data| Self { data }).ok()
    }
}

#[cfg(feature = "mmap")]
pub mod mmap {
    use super::*;

    use memmap2::Mmap;

    use crate::MdfRecords;

    pub struct MemMap(Mmap);

    impl AsRef<[u32]> for MemMap {
        fn as_ref(&self) -> &[u32] {
            bytemuck::try_cast_slice(self.0.as_ref()).expect("alignment matches, length compatible")
        }
    }

    impl MdfRecords<MemMap> {
        pub fn mmap_file(file: impl AsRef<Path>) -> IoResult<Self> {
            let file = File::open(file)?;
            let map = unsafe { Mmap::map(&file) }?;
            Ok(MdfRecords { data: MemMap(map) })
        }
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

        let (record, rest) = MdfRecordRef::from_data(self.data).expect("valid mdf data");
        self.data = rest;

        Some(record)
    }
}

fn truncate_data<'a>(data: &'a [impl Debug]) -> Box<dyn Debug + 'a> {
    if data.len() < 20 {
        Box::new(data)
    } else {
        let mut output = String::new();
        output.push_str("[ ");
        for d in &data[0..10] {
            output.push_str(&format!("{d:?}"));
            output.push_str(", ");
        }
        output.push_str("...");
        output.push_str(" ]");

        Box::new(output)
    }
}

impl<D: AsRef<[u32]>> Debug for MdfRecords<D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list()
            .entries(self.mdf_record_iter().map(|r| {
                r.try_into_single_event()
                    .map(|r| r as &dyn Debug)
                    .unwrap_or(r)
            }))
            .finish()
    }
}

#[cfg(test)]
mod test {

    use include_bytes_aligned::include_bytes_aligned;

    use crate::MdfRecords;

    #[test]
    #[ignore]
    fn print_data() {
        let file = include_bytes!("../test.mdf");
        // let file = include_bytes!("../../../truc.mdf");
        let records = MdfRecords::from_data(file);
        println!("{:#?}", records);
    }

    #[test]
    fn bin_read_test() {
        let file = include_bytes!("../test.mdf");
        let mut cursor = &file[..];

        while !cursor.is_empty() {
            let size = u32::from_le_bytes(cursor[0..4].try_into().unwrap());
            let size2 = u32::from_le_bytes(cursor[4..8].try_into().unwrap());
            let size3 = u32::from_le_bytes(cursor[8..12].try_into().unwrap());

            println!("{size}, {size2}, {size3}");

            assert_eq!(size, size2);
            assert_eq!(size2, size3);

            cursor = &cursor[size as usize..];
        }
    }

    #[test]
    #[ignore]
    fn some_size() {
        let data = include_bytes_aligned!(4, "../test.mdf");
        let mdfs = MdfRecords::from_aligned_slice(data).expect("aligned");
        let size: usize = mdfs
            .mdf_record_iter()
            .skip(213)
            .take(2)
            .map(|x| x.size_bytes())
            .sum();
        println!("{size}");
    }

    #[test]
    fn test_file() {
        let records = MdfRecords::read_file("test.mdf").unwrap();
        println!("{}", records.mdf_record_iter().count());
    }

    #[test]
    #[cfg(feature = "mmap")]
    fn test_mmap() {
        let records = MdfRecords::mmap_file("test.mdf").unwrap();
        println!("{}", records.mdf_record_iter().count());
    }
}
