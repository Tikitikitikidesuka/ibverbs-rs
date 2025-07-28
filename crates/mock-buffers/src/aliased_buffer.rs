use circular_buffer::{CircularBufferReader, CircularBufferWriter};
use thiserror::Error;

pub struct MockAliasedBuffer {
    alignment_pow2: u8,
    read_ptr: usize,
    write_ptr: usize,
    same_page: bool,
    buffer: Vec<u8>, // Double the requested capacity for simulated aliasing
}

impl MockAliasedBuffer {
    pub fn new(capacity: usize, alignment_pow2: u8) -> Result<Self, ()> {
        if !alignment_utils::check_alignment_pow2(capacity, alignment_pow2) {
            Err(())
        } else {
            Ok(Self {
                alignment_pow2,
                read_ptr: 0,
                write_ptr: 0,
                same_page: true,
                buffer: vec![0; capacity * 2], // Double size for aliasing
            })
        }
    }

    // Replicate the aliased memory for a given range
    fn replicate_alias(&mut self, write_ptr: usize, size: usize) {
        let real_capacity = self.buffer.len() / 2;

        // If written to just real buffer
        if write_ptr + size < real_capacity {
            let (real_buffer, aliased_buffer) = self.buffer.split_at_mut(real_capacity);
            let written_region = &real_buffer[write_ptr..write_ptr + size];
            aliased_buffer[write_ptr..write_ptr + size].copy_from_slice(written_region);
        }
        // If write region crossed into aliased region
        else {
            let (real_buffer, aliased_buffer) = self.buffer.split_at_mut(real_capacity);

            // Copy real buffer written part to aliased buffer
            let written_region = &real_buffer[write_ptr..];
            aliased_buffer[write_ptr..].copy_from_slice(written_region);

            // Copy aliased buffer written part to real buffer
            let written_region = &aliased_buffer[..(write_ptr + size) % real_capacity];
            real_buffer[..(write_ptr + size) % real_capacity].copy_from_slice(written_region);
        }
    }
}

pub struct MockAliasedBufferReader {
    buffer: *mut MockAliasedBuffer,
}

pub struct MockAliasedBufferWriter {
    buffer: *mut MockAliasedBuffer,
}

impl MockAliasedBufferReader {
    pub fn new(buffer: &mut MockAliasedBuffer) -> Self {
        Self { buffer }
    }

    pub fn alignment_pow2(&self) -> u8 {
        unsafe { &*self.buffer }.alignment_pow2
    }
}

impl MockAliasedBufferWriter {
    pub fn new(buffer: &mut MockAliasedBuffer) -> Self {
        Self { buffer }
    }

    pub fn alignment_pow2(&self) -> u8 {
        unsafe { &*self.buffer }.alignment_pow2
    }
}

#[derive(Debug, Error)]
pub enum AliasedBufferAdvanceError {
    #[error("Not enough data available")]
    OutOfBounds,
    #[error("Result address not aligned")]
    NotAligned,
}

impl CircularBufferReader for MockAliasedBufferReader {
    type AdvanceResult = Result<(), AliasedBufferAdvanceError>;
    type ReadableRegionResult<'a> = &'a [u8];

    fn advance_read_pointer(&mut self, bytes: usize) -> Self::AdvanceResult {
        let buf = unsafe { &mut *self.buffer };

        // Check alignment
        if !alignment_utils::check_alignment_pow2(bytes, buf.alignment_pow2) {
            return Err(AliasedBufferAdvanceError::NotAligned);
        }

        // Check enough data available
        let available = self.readable_region().len();
        if bytes > available {
            return Err(AliasedBufferAdvanceError::OutOfBounds);
        }

        // Handle wrapping when advancing
        let capacity = buf.buffer.len() / 2;
        if buf.read_ptr + bytes >= capacity {
            buf.read_ptr = (buf.read_ptr + bytes) % capacity;
            buf.same_page = !buf.same_page;
        } else {
            buf.read_ptr += bytes;
        }

        Ok(())
    }

    fn readable_region(&self) -> Self::ReadableRegionResult<'_> {
        let buf = unsafe { &*self.buffer };

        let available = if buf.same_page {
            buf.write_ptr - buf.read_ptr
        } else {
            let capacity = buf.buffer.len() / 2;
            capacity - buf.read_ptr + buf.write_ptr
        };

        &buf.buffer[buf.read_ptr..buf.read_ptr + available]
    }
}

impl CircularBufferWriter for MockAliasedBufferWriter {
    type AdvanceResult = Result<(), AliasedBufferAdvanceError>;
    type WriteableRegionResult<'a> = &'a mut [u8];

    fn advance_write_pointer(&mut self, bytes: usize) -> Self::AdvanceResult {
        let buf = unsafe { &mut *self.buffer };

        // Check alignment
        if !alignment_utils::check_alignment_pow2(bytes, buf.alignment_pow2) {
            return Err(AliasedBufferAdvanceError::NotAligned);
        }

        // Check enough space available
        let available = self.writable_region().len();
        if bytes > available {
            return Err(AliasedBufferAdvanceError::OutOfBounds);
        }

        // CRITICAL: Replicate the alias
        buf.replicate_alias(buf.write_ptr, bytes);

        // Handle wrapping when advancing
        let capacity = buf.buffer.len() / 2;
        if buf.write_ptr + bytes >= capacity {
            buf.write_ptr = (buf.write_ptr + bytes) % capacity;
            buf.same_page = !buf.same_page;
        } else {
            buf.write_ptr += bytes;
        }

        Ok(())
    }

    fn writable_region(&mut self) -> Self::WriteableRegionResult<'_> {
        let buf = unsafe { &mut *self.buffer };

        let available = if buf.same_page {
            let capacity = buf.buffer.len() / 2;
            capacity - buf.write_ptr + buf.read_ptr
        } else {
            buf.read_ptr - buf.write_ptr
        };

        &mut buf.buffer[buf.write_ptr..buf.write_ptr + available]
    }
}
