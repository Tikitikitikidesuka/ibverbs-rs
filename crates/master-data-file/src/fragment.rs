//! Bank (aka fragment) of an MDF record.

use core::slice;
use std::{fmt::Debug, io::Write};

use bytemuck::{Pod, Zeroable, cast_ref};
use std::io::Result as IoResult;
use ebutils::{EventId, Uninstantiatable, fragment::Fragment, source_id::SourceId};

use crate::{MdfFromDataError, truncate_data, writer::WriteMdf};

#[repr(C, align(4))]
#[derive(Copy, Clone, Pod, Zeroable, Debug)]
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

impl<'a> WriteMdf for Fragment<'a> {
    fn write_mdf(&self, writer: &mut impl Write) -> IoResult<()> {
        let header_size: u16 = size_of::<MdfFragmentHeader>()
            .try_into()
            .expect("header size fits u16");
        let header = MdfFragmentHeader {
            magic: MdfFragmentHeader::MAGIC,
            fragment_type: self.fragment_type_raw(),
            source_id: self.source_id(),
            version: self.version(),

            size: header_size + self.fragment_size(),
        };
        writer.write_all(header.as_bytes())?;
        writer.write_all(self.payload())?;

        // pad to u32 size
        let frag_size = self.fragment_size() as usize;
        let padding = frag_size.next_multiple_of(align_of::<u32>()) - frag_size;
        writer.write_all(&0u32.to_ne_bytes()[..padding])?;

        Ok(())
    }
}

#[repr(align(4))]
/// Aka bank.
///
/// May only ever exist as `&MdfFragment`.
// todo add an external type once they stabilize github.com/rust-lang/rust/issues/43467
pub(crate) struct MdfFragment {
    header: MdfFragmentHeader,
    _unin: Uninstantiatable,
}

impl Debug for MdfFragment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MdfFragment")
            .field("header", &self.header)
            .field("data", &truncate_data(self.data()))
            .finish()
    }
}

impl MdfFragment {
    /// `slice` needs to contain at least one full correct MDF.
    /// `slice` may be larger towards the end.
    pub fn from_data(slice: &[u32]) -> Result<(&Self, &[u32]), MdfFromDataError> {
        const HEADER_SIZE_U32: usize = size_of::<MdfFragmentHeader>() / size_of::<u32>();
        let header_data: &[u32; HEADER_SIZE_U32] = slice
            .split_at_checked(HEADER_SIZE_U32)
            .ok_or(MdfFromDataError::TooSmallForHeader(slice.len()))?
            .0
            .try_into()
            .expect("size matches"); // todo different error
        let header: &MdfFragmentHeader = cast_ref(header_data);

        let length_u32 = (header.size as usize).div_ceil(size_of::<u32>());
        if slice.len() < length_u32 {
            return Err(MdfFromDataError::TotalLengthMismatch {
                expected: length_u32,
                got: slice.len(),
            });
        }

        let fragment = unsafe { &*slice.as_ptr().cast() };

        Ok((fragment, &slice[length_u32..]))
    }

    pub fn data(&self) -> &[u8] {
        let offset = size_of_val(&self.header);
        unsafe {
            slice::from_raw_parts(
                (self as *const Self as *const u8).byte_add(offset),
                self.header.size as usize - offset,
            )
        }
    }

    pub fn as_fragment(&self, event_id: EventId) -> Fragment<'_> {
        Fragment::new(
            self.header.fragment_type,
            self.header.version,
            event_id,
            self.header.source_id,
            self.data(),
        )
    }
}
