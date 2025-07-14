use crate::circular_buffer::CircularBufferWriter;
use crate::shared_memory_buffer::buffer_backend::SharedMemoryWriteBuffer;
use crate::shared_memory_buffer::buffer_status::PtrStatus;
use crate::shared_memory_buffer::reader::SharedMemoryBufferAdvanceError;
use crate::utils;
use tracing::{debug, instrument, warn};

pub struct SharedMemoryBufferWriter {
    buffer: SharedMemoryWriteBuffer,
    write_status: PtrStatus,
}

impl SharedMemoryBufferWriter {
    pub fn new(buffer: SharedMemoryWriteBuffer) -> Self {
        debug!("Creating new shared memory buffer writer");
        let write_status = buffer.write_status();

        Self {
            buffer,
            write_status,
        }
    }

    pub fn alignment_pow2(&self) -> u8 {
        self.buffer.alignment_pow2()
    }
}

impl CircularBufferWriter for SharedMemoryBufferWriter {
    type AdvanceResult = Result<(), SharedMemoryBufferAdvanceError>;
    type WriteableRegionResult<'a> = (&'a mut [u8], &'a mut [u8]);

    #[instrument(skip_all, fields(shmem = self.buffer.name(), bytes = bytes))]
    fn advance_write_pointer(&mut self, bytes: usize) -> Self::AdvanceResult {
        debug!("Attempting to advance the buffer's write pointer by {bytes} bytes");

        debug!("Checking minimum 2 byte alignment due to pointer representation");
        if !utils::check_alignment_pow2(bytes, 1) {
            warn!("Aborting write pointer advance due to failed 2 byte alignment violation");
            return Err(SharedMemoryBufferAdvanceError::Not2ByteAligned);
        }

        debug!("Checking buffer's alignment");
        if !utils::check_alignment_pow2(bytes, self.buffer.alignment_pow2()) {
            warn!("Aborting write pointer advance due to buffer's alignment violation");
            return Err(SharedMemoryBufferAdvanceError::NotAligned);
        }

        debug!("Checking buffer's available space on writable region");
        let (primary_region, secondary_region) = self.writable_region();
        let available = primary_region.len() + secondary_region.len();
        if bytes > available {
            warn!("Aborting write pointer advance due to insufficient buffer writable region space");
            return Err(SharedMemoryBufferAdvanceError::OutOfBounds);
        }

        debug!("All necessary checks passed for write pointer advance passed! Updating write pointer");
        self.write_status = self.write_status.plus(bytes, self.buffer.size());
        self.buffer.set_write_status(self.write_status);

        Ok(())
    }

    #[instrument(skip_all, fields(shmem = self.buffer.name()))]
    fn writable_region(&mut self) -> Self::WriteableRegionResult<'_> {
        debug!("Attempting to get the buffer's writable region");

        debug!("Getting the buffer's read pointer and complete byte slice");
        let read_status = self.buffer.read_status();
        let buffer_slice = unsafe { self.buffer.as_slice_mut() };

        debug!("Checking if the read and write pointer are on the same page");
        // Being on different pages means only one of the two has wrapped around
        let same_page = self.write_status.wrap() == read_status.wrap();

        if same_page {
            debug!("Writable region wraps around");
            // Primary region: from write_ptr to end
            // Secondary region: from start to read_ptr
            let (before_read, after_read) = buffer_slice.split_at_mut(read_status.ptr() as usize);
            let primary_region =
                &mut after_read[(self.write_status.ptr() as usize - read_status.ptr() as usize)..];

            debug!("Got the two regions successfully");
            (primary_region, before_read)
        } else {
            debug!("Writable region does no wrap around");
            // Primary region: from write_ptr to read_ptr
            // Secondary region: empty
            let primary_region =
                &mut buffer_slice[self.write_status.ptr() as usize..read_status.ptr() as usize];

            debug!("Got the primary region and put an empty slice on secondary successfully");
            (primary_region, &mut [])
        }
    }
}
