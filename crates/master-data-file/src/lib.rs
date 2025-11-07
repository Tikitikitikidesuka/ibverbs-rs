use core::fmt;
use std::{fmt::Debug, slice};

use bytemuck::{cast_ref, cast_slice_mut};
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

pub struct MdfRecords {
    data: Box<[u32]>,
}
impl MdfRecords {
    /// # Safety
    /// Data must contain valid mdf records.
    /// Data will be copied to ensure alignment.
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
        println!("remaining length {}", self.data.len());

        let (record, rest) = MdfRecordRef::from_data(self.data).expect("valid mdf data");
        self.data = rest;

        // println!(
        //     "data around start {:?} (record size {})",
        //     &self.data[(record.size_u32() as usize).saturating_sub(64)
        //         ..(record.size_u32() as usize) + 64],
        //     record.size_u32()
        // );
        Some(record)
    }
}

fn truncate_data<'a>(data: &'a [impl Debug]) -> Box<dyn Debug + 'a> {
    if data.len() < 100 {
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

impl Debug for MdfRecords {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list()
            .entries(
                self.mdf_record_iter(), /*.map(|r| {
                                            r.try_into_single_event()
                                                .map(|r| r as &dyn Debug)
                                                .unwrap_or(r)
                                        })*/
            )
            .finish()
    }
}

#[cfg(test)]
mod test {

    use crate::MdfRecords;

    #[test]
    fn test_file() {
        let file = include_bytes!("../../../Run_0000328614_20250828-135252-159_TDEB03_0017.mdf");
        // let file = include_bytes!("../../../truc.mdf");
        let records = unsafe { MdfRecords::from_data(file) };
        println!("{records:#?}");
    }

    #[test]
    fn bin_read_test() {
        let file = include_bytes!("../../../Run_0000328614_20250828-135252-159_TDEB03_0017.mdf");
        // let mut cursor = &file[..];

        let mut start = 0;

        // let mut chunks = Vec::new();
        while file.get(start).is_some() {
            let size = u32::from_le_bytes(file[start..start + 4].try_into().unwrap());
            let size2 = u32::from_le_bytes(file[start + 4..start + 8].try_into().unwrap());
            let size3 = u32::from_le_bytes(file[start + 8..start + 12].try_into().unwrap());

            println!("{size}, {size2}, {size3}");
            // if size != 800 && false {
            //     println!("not 800!");
            //     dbg!(&file[size as usize - 100..size as usize + 100]);
            //     // println!(
            //     //     "{:#?} -- {:?}",
            //     //     chunks
            //     //         .iter()
            //     //         .rev()
            //     //         .take(3)
            //     //         .copied()
            //     //         .map(to_u32)
            //     //         .collect::<Vec<_>>(),
            //     //     cursor
            //     //         .chunks(4)
            //     //         .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            //     //         .take(50)
            //     //         .collect::<Vec<_>>()
            //     // );
            //     panic!();
            // }
            // let previous = &cursor[..u32 as usize * size_of::<u32>()];
            // chunks.push(previous);
            start += size as usize;
            // cursor = &cursor[size as usize * size_of::<u32>()..];
        }
    }

    // fn to_u32(slice: &[u8]) -> Vec<u32> {
    //     slice
    //         .chunks(4)
    //         .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
    //         .collect()
    // }
}
