use crate::circular_buffer::CircularBufferReader;
use crate::shared_memory_buffer::buffer_backend::SharedMemoryReadBuffer;
use crate::shared_memory_buffer::buffer_status::PtrStatus;
use crate::utils;
use log::error;
use thiserror::Error;

pub struct SharedMemoryBufferReader {
    buffer: SharedMemoryReadBuffer,
    read_status: PtrStatus,
}

#[derive(Debug, Error)]
pub enum SharedMemoryBufferAdvanceError {
    #[error("Not enough data available")]
    OutOfBounds,

    #[error("Result address not aligned")]
    NotAligned,

    #[error("Result address is not minimum 2 Byte aligned")]
    Not2ByteAligned,
}

impl SharedMemoryBufferReader {
    pub fn new(read_buffer: SharedMemoryReadBuffer) -> Self {
        let read_status = read_buffer.read_status();

        Self {
            buffer: read_buffer,
            read_status,
        }
    }

    pub fn alignment_pow2(&self) -> u8 {
        self.buffer.alignment_pow2()
    }
}

impl CircularBufferReader for SharedMemoryBufferReader {
    type AdvanceResult = Result<(), SharedMemoryBufferAdvanceError>;
    type ReadableRegionResult<'a> = (&'a [u8], &'a [u8]);

    fn advance_read_pointer(&mut self, bytes: usize) -> Self::AdvanceResult {
        // Check minimum 2 byte alignment due to pointer representation
        if !utils::check_alignment_pow2(bytes, 1) {
            return Err(SharedMemoryBufferAdvanceError::Not2ByteAligned);
        }

        // Check alignment
        if !utils::check_alignment_pow2(bytes, self.buffer.alignment_pow2()) {
            return Err(SharedMemoryBufferAdvanceError::NotAligned);
        }

        // Check enough data available
        let (primary_region, secondary_region) = self.readable_region();
        let available = primary_region.len() + secondary_region.len();
        if bytes > available {
            return Err(SharedMemoryBufferAdvanceError::OutOfBounds);
        }

        // Update read status and handle wrapping when advancing
        self.read_status = self.read_status.plus(bytes, self.buffer.size());
        self.buffer.set_read_status(self.read_status);

        Ok(())
    }

    fn readable_region(&self) -> Self::ReadableRegionResult<'_> {
        let write_status = self.buffer.write_status();
        let buffer_slice = unsafe { self.buffer.as_slice() };

        let same_page = write_status.wrap() == self.read_status.wrap();

        if same_page {
            // No wraparound -> Primary: from read_ptr to write_ptr, Secondary: empty
            let primary_region =
                &buffer_slice[self.read_status.ptr() as usize..write_status.ptr() as usize];
            let secondary_region = &[];
            (primary_region, secondary_region)
        } else {
            // Wraparound -> Primary: from read_ptr to end, Secondary: from start to write_ptr
            let primary_region = &buffer_slice[self.read_status.ptr() as usize..];
            let secondary_region = &buffer_slice[..write_status.ptr() as usize];
            (primary_region, secondary_region)
        }
    }
}
