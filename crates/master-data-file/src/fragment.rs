//! Bank (aka fragment) of an MDF record.

use core::slice;
use std::io::Write;

use bytemuck::NoUninit;
use multi_fragment_packet::{FragmentRef, SourceId};
use std::io::Result as IoResult;
use typed_builder::TypedBuilder;


#[repr(C, align(4))]
#[derive(Copy, Clone, NoUninit)]
pub struct MdfFragmentHeader {
    magic: u16,
    /// size in bytes including header without padding
    size: u16,
    fragment_type: u8,
    version: u8,
    source_id: SourceId,
}

impl MdfFragmentHeader {
    pub const MAGIC: u16 = 0xCBCB;

    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::bytes_of(self)
    }
}

#[derive(TypedBuilder)]
pub struct MdfFragmentWriter<'a> {
    version: u8,
    source_id: SourceId,
    fragment: &'a FragmentRef<'a>,
}

impl<'a> MdfFragmentWriter<'a> {
    pub fn write(&self, writer: &mut impl Write) -> IoResult<()> {
        let header = MdfFragmentHeader {
            magic: MdfFragmentHeader::MAGIC,
            fragment_type: self.fragment.fragment_type(),
            source_id: self.source_id,
            version: self.version,

            size: size_of::<MdfFragmentHeader>() as u16 + self.fragment.fragment_size(),
        };
        writer.write_all(header.as_bytes())?;
        writer.write_all(self.fragment.data())
    }
}

#[repr(align(4))]
pub struct MdfFragmentRef {
    header: MdfFragmentHeader,
}

impl MdfFragmentRef {
    /// ## Safety
    /// `slice` needs to contain at least one full correct MDF.
    /// `slice` may be larger towards the end.
    pub unsafe fn from_raw(slice: &[u32]) -> &Self {
        assert!(!slice.is_empty());
        unsafe { &*slice.as_ptr().cast() }
    }

    pub fn bank_type(&self) -> u8 {
        self.header.fragment_type
    }

    pub fn version(&self) -> u8 {
        self.header.version
    }

    pub fn source_id(&self) -> SourceId {
        self.header.source_id
    }

    // Size in bytes including the header, without padding to u32 size.
    pub fn size_bytes(&self) -> usize {
        self.header.size as _
    }

    pub fn size_bytes_padded(&self) -> usize {
        self.size_bytes().next_multiple_of(align_of::<u32>())
    }

    pub fn data(&self) -> &[u8] {
        let offset = size_of_val(&self.header);
        unsafe {
            slice::from_raw_parts(
                (self as *const Self as *const u8).byte_add(offset),
                self.size_bytes() - offset,
            )
        }
    }
}
