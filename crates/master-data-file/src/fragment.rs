//! Bank (aka fragment) of an MDF record.

use core::slice;

use multi_fragment_packet::SourceId;

#[repr(C, align(4))]
pub struct MdfFragmentHeader {
    magic: u16,
    /// size in bytes including header without padding
    size: u16,
    bank_type: u8,
    version: u8,
    source_id: SourceId,
}

#[repr(align(4))]
pub struct MdfFragmentRef {
    header: MdfFragmentHeader,
}

impl MdfFragmentRef {
    pub const MAGIC: u16 = 0xCBCB;

    /// ## Safety
    /// `slice` needs to contain at least one full correct MDF.
    /// `slice` may be larger towards the end.
    pub unsafe fn from_raw(slice: &[u32]) -> &Self {
        assert!(!slice.is_empty());
        unsafe { &*slice.as_ptr().cast() }
    }

    pub fn bank_type(&self) -> u8 {
        self.header.bank_type
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
