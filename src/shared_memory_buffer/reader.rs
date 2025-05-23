use std::cmp::min;
use crate::shared_memory_buffer::buffer_backend::SharedMemoryReadBuffer;
use crate::zero_copy_ring_buffer_reader::{ZeroCopyRingBufferReader, ZeroCopyRingBufferReaderError};

struct SharedMemoryBufferReader {
    buffer: SharedMemoryReadBuffer,
    local_read_ptr: usize,
    local_write_ptr: usize,
}

/*
impl ZeroCopyRingBufferReader for SharedMemoryBufferReader {
    unsafe fn unsafe_data(&self) -> &[u8] {
        &self.buffer.as_slice()[self.local_read_ptr..self.local_write_ptr]
    }

    fn load_all_data(&mut self) -> Result<usize, ZeroCopyRingBufferReaderError> {
        let prev_write_ptr = self.local_write_ptr;
        self.local_write_ptr = self.buffer.write_ptr() as usize;
        Ok(prev_write_ptr - self.local_write_ptr)
    }

    fn discard_data(&mut self, num_bytes: usize) -> Result<usize, ZeroCopyRingBufferReaderError> {
        let new_read_ptr = min(self.local_write_ptr, self.local_read_ptr + num_bytes);
        let discarded_bytes = new_read_ptr - self.local_write_ptr;
        self.local_read_ptr += discarded_bytes;
        self.buffer.set_read_ptr(self.local_read_ptr as u64);
        Ok(discarded_bytes)
    }

    fn discard_all_data(&mut self) -> Result<usize, ZeroCopyRingBufferReaderError> {
        let discarded_bytes = self.local_write_ptr - self.local_write_ptr;
        self.local_read_ptr = self.local_write_ptr;
        self.buffer.set_read_ptr(self.local_read_ptr as u64);
        Ok(discarded_bytes)
    }

    fn alignment(&self) -> Result<Option<usize>, ZeroCopyRingBufferReaderError> {
        Ok(Some(1 << self.buffer.alignment_2pow()))
    }
}
*/