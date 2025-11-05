use std::{mem, slice};

use bytemuck::{NoUninit, bytes_of, cast_slice_mut};

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
        assert_eq!(self.generic_header.length_1, self.generic_header.length_2);
        assert_eq!(self.generic_header.length_2, self.generic_header.length_3);
        self.generic_header.length_1
    }

    /// Returns the entire record length in bytes.
    pub fn size_bytes(&self) -> usize {
        self.size_u32() as usize * size_of::<u32>()
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

impl MdfRecords {
    /// # Safety
    /// Data must contain valid mdf records
    pub unsafe fn from_data(data: &[u8]) -> Self {
        let mut boxed = vec![0u32; data.len().div_ceil(size_of::<u32>())].into_boxed_slice();
        cast_slice_mut(&mut boxed)[0..data.len()].copy_from_slice(data);
        Self { data: boxed }
    }

    pub fn mdf_record_iter(&self) -> MdfRecordIterator<'_> {
        todo!()
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

        let record: Self::Item = unsafe { &*self.data.as_ptr().cast() };
        self.data = &self.data[record.size_u32() as _..];
        Some(record)
    }
}
