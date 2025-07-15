use crate::circular_buffer::CircularBufferReader;
use crate::shared_memory_buffer::buffer_backend::SharedMemoryReadBuffer;
use crate::shared_memory_buffer::buffer_status::PtrStatus;
use crate::utils;
use thiserror::Error;
use tracing::{debug, instrument, warn};

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
        debug!("Creating new shared memory buffer reader");
        let read_status = read_buffer.read_status();

        Self {
            buffer: read_buffer,
            read_status,
        }
    }

    pub fn alignment_pow2(&self) -> u8 {
        self.buffer.alignment_pow2()
    }

    pub fn buffer_name(&self) -> &str {
        self.buffer.name()
    }
}

impl CircularBufferReader for SharedMemoryBufferReader {
    type AdvanceResult = Result<(), SharedMemoryBufferAdvanceError>;
    type ReadableRegionResult<'a> = (&'a [u8], &'a [u8]);

    #[instrument(skip_all, fields(shmem = self.buffer.name(), bytes = bytes))]
    fn advance_read_pointer(&mut self, bytes: usize) -> Self::AdvanceResult {
        debug!("Attempting to advance the buffer's read pointer by {bytes} bytes");

        debug!("Checking minimum 2 byte alignment due to pointer representation");
        if !utils::check_alignment_pow2(bytes, 1) {
            warn!("Aborting read pointer advance due to failed 2 byte alignment violation");
            return Err(SharedMemoryBufferAdvanceError::Not2ByteAligned);
        }

        debug!("Checking buffer's alignment");
        if !utils::check_alignment_pow2(bytes, self.buffer.alignment_pow2()) {
            warn!("Aborting write pointer advance due to buffer's alignment violation");
            return Err(SharedMemoryBufferAdvanceError::NotAligned);
        }

        debug!("Checking buffer's available space on readable region");
        let (primary_region, secondary_region) = self.readable_region();
        let available = primary_region.len() + secondary_region.len();
        if bytes > available {
            warn!("Aborting read pointer advance due to insufficient buffer readable region space");
            return Err(SharedMemoryBufferAdvanceError::OutOfBounds);
        }

        debug!("All necessary checks for read pointer advance passed! Updating read pointer");
        self.read_status = self.read_status.plus(bytes, self.buffer.size());
        self.buffer.set_read_status(self.read_status);

        Ok(())
    }

    #[instrument(skip_all, fields(shmem = self.buffer.name()))]
    fn readable_region(&self) -> Self::ReadableRegionResult<'_> {
        debug!("Attempting to get the buffer's readable region");

        debug!("Getting the buffer's write pointer and complete byte slice");
        let write_status = self.buffer.write_status();
        let buffer_slice = unsafe { self.buffer.as_slice() };

        debug!("Checking if the read and write pointer are on the same page");
        // Being on different pages means only one of the two has wrapped around
        let same_page = write_status.wrap() == self.read_status.wrap();

        if same_page {
            debug!("Readable region does no wrap around");
            // Primary region: from read_ptr to write_ptr
            // Secondary region: empty
            let primary_region =
                &buffer_slice[self.read_status.ptr() as usize..write_status.ptr() as usize];
            let secondary_region = &[];

            debug!("Got the primary region and put an empty slice on secondary successfully");
            (primary_region, secondary_region)
        } else {
            debug!("Readable region wraps around");
            // Primary region: from read_ptr to end
            // Secondary region: from start to write_ptr
            let primary_region = &buffer_slice[self.read_status.ptr() as usize..];
            let secondary_region = &buffer_slice[..write_status.ptr() as usize];

            debug!("Got the two regions successfully");
            (primary_region, secondary_region)
        }
    }
}
