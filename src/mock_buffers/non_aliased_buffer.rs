use crate::utils;
use thiserror::Error;
use crate::circular_buffer::{CircularBufferReader, CircularBufferWriter};

pub struct MockNonAliasedBuffer {
    alignment_pow2: u8,
    read_ptr: usize,
    write_ptr: usize,
    same_page: bool,
    buffer: Vec<u8>,
}

impl MockNonAliasedBuffer {
    pub fn new(capacity: usize, alignment_pow2: u8) -> Result<Self, ()> {
        if !utils::check_alignment_pow2(capacity, alignment_pow2) {
            Err(())
        } else {
            Ok(Self {
                alignment_pow2,
                read_ptr: 0,
                write_ptr: 0,
                same_page: true,
                buffer: vec![0; capacity],
            })
        }
    }
}

pub struct MockNonAliasedBufferReader {
    buffer: *mut MockNonAliasedBuffer,
}

pub struct MockNonAliasedBufferWriter {
    buffer: *mut MockNonAliasedBuffer,
}

impl MockNonAliasedBufferReader {
    pub fn new(buffer: &mut MockNonAliasedBuffer) -> Self {
        Self { buffer }
    }

    pub fn alignment_pow2(&self) -> u8 {
        unsafe { &*self.buffer }.alignment_pow2
    }
}

impl MockNonAliasedBufferWriter {
    pub fn new(buffer: &mut MockNonAliasedBuffer) -> Self {
        Self { buffer }
    }

    pub fn alignment_pow2(&self) -> u8 {
        unsafe { &*self.buffer }.alignment_pow2
    }
}

#[derive(Debug, Error)]
pub enum MockNonAliasedAdvanceError {
    #[error("Not enough data available")]
    OutOfBounds,
    #[error("Result address not aligned")]
    NotAligned,
}

impl CircularBufferReader for MockNonAliasedBufferReader {
    type AdvanceResult = Result<(), MockNonAliasedAdvanceError>;
    type ReadableRegionResult<'a> = (&'a [u8], &'a [u8]);

    fn advance_read_pointer(&mut self, bytes: usize) -> Self::AdvanceResult {
        let buf = unsafe { &mut *self.buffer };

        // Check alignment
        if !utils::check_alignment_pow2(bytes, buf.alignment_pow2) {
            return Err(MockNonAliasedAdvanceError::NotAligned);
        }

        // Check enough data available
        let (primary_region, secondary_region) = self.readable_region();
        let available = primary_region.len() + secondary_region.len();
        if bytes > available {
            return Err(MockNonAliasedAdvanceError::OutOfBounds);
        }

        // Handle wrapping when advancing
        if buf.read_ptr + bytes >= buf.buffer.len() {
            buf.read_ptr = (buf.read_ptr + bytes) % buf.buffer.len();
            buf.same_page = !buf.same_page;
        } else {
            buf.read_ptr += bytes;
        }

        Ok(())
    }

    fn readable_region(&self) -> Self::ReadableRegionResult<'_> {
        let buf = unsafe { &*self.buffer };

        if buf.same_page {
            // Primary: from read_ptr to write_ptr, Secondary: empty
            let primary_region = &buf.buffer[buf.read_ptr..buf.write_ptr];
            (primary_region, &[])
        } else {
            // Primary: from read_ptr to end, Secondary: from start to write_ptr
            let primary_region = &buf.buffer[buf.read_ptr..];
            let secondary_region = &buf.buffer[..buf.write_ptr];
            (primary_region, secondary_region)
        }
    }
}

impl CircularBufferWriter for MockNonAliasedBufferWriter {
    type AdvanceResult = Result<(), MockNonAliasedAdvanceError>;
    type WriteableRegionResult<'a> = (&'a mut [u8], &'a mut [u8]);

    fn advance_write_pointer(&mut self, bytes: usize) -> Self::AdvanceResult {
        let buf = unsafe { &mut *self.buffer };

        // Check alignment
        if !utils::check_alignment_pow2(bytes, buf.alignment_pow2) {
            return Err(MockNonAliasedAdvanceError::NotAligned);
        }

        // Check enough data available
        let (primary_region, secondary_region) = self.writable_region();
        let available = primary_region.len() + secondary_region.len();
        if bytes > available {
            return Err(MockNonAliasedAdvanceError::OutOfBounds);
        }

        // Handle wrapping when advancing
        if buf.write_ptr + bytes >= buf.buffer.len() {
            buf.write_ptr = (buf.write_ptr + bytes) % buf.buffer.len();
            buf.same_page = !buf.same_page;
        } else {
            buf.write_ptr += bytes;
        }

        Ok(())
    }

    fn writable_region(&mut self) -> Self::WriteableRegionResult<'_> {
        let buf = unsafe { &mut *self.buffer };

        if buf.same_page {
            // Primary: from write_ptr to end, Secondary: from start to read_ptr
            let (before_read, after_read) = buf.buffer.split_at_mut(buf.read_ptr);
            let primary_region = &mut after_read[buf.write_ptr - buf.read_ptr..];
            (primary_region, before_read)
        } else {
            // Primary: from write_ptr to read_ptr, Secondary: empty
            let primary_region = &mut buf.buffer[buf.write_ptr..buf.read_ptr];
            (primary_region, &mut [])
        }
    }
}
