use crate::circular_buffer::CircularBufferWriter;
use crate::shared_memory_buffer::buffer_backend::SharedMemoryWriteBuffer;
use crate::shared_memory_buffer::buffer_status::PtrStatus;
use crate::shared_memory_buffer::reader::SharedMemoryBufferAdvanceError;
use crate::utils;

pub struct SharedMemoryBufferWriter {
    buffer: SharedMemoryWriteBuffer,
    write_status: PtrStatus,
}

impl SharedMemoryBufferWriter {
    pub fn new(buffer: SharedMemoryWriteBuffer) -> Self {
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

    fn advance_write_pointer(&mut self, bytes: usize) -> Self::AdvanceResult {
        // Check minimum 2 byte alignment due to pointer representation
        if !utils::check_alignment_pow2(bytes, 1) {
            return Err(SharedMemoryBufferAdvanceError::Not2ByteAligned);
        }

        // Check alignment
        if !utils::check_alignment_pow2(bytes, self.buffer.alignment_pow2()) {
            return Err(SharedMemoryBufferAdvanceError::NotAligned);
        }

        // Check enough data available
        let (primary_region, secondary_region) = self.writable_region();
        let available = primary_region.len() + secondary_region.len();
        if bytes > available {
            return Err(SharedMemoryBufferAdvanceError::OutOfBounds);
        }

        // Update read status and handle wrapping when advancing
        self.write_status = self.write_status.plus(bytes, self.buffer.size());
        self.buffer.set_write_status(self.write_status);

        Ok(())
    }

    fn writable_region(&mut self) -> Self::WriteableRegionResult<'_> {
        let read_status = self.buffer.read_status();
        let buffer_slice = unsafe { self.buffer.as_slice_mut() };

        let same_page = self.write_status.wrap() == read_status.wrap();

        if same_page {
            // Wraparound -> Primary: from write_ptr to end, Secondary: from start to read_ptr
            let (before_read, after_read) = buffer_slice.split_at_mut(read_status.ptr() as usize);
            let primary_region =
                &mut after_read[(self.write_status.ptr() as usize - read_status.ptr() as usize)..];
            (primary_region, before_read)
        } else {
            // No wraparound -> Primary: from write_ptr to read_ptr, Secondary: empty
            let primary_region =
                &mut buffer_slice[self.write_status.ptr() as usize..read_status.ptr() as usize];
            (primary_region, &mut [])
        }
    }
}
