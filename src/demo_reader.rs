use std::cmp::min;
use crate::zero_copy_ring_buffer_reader::{
    DataGuard, ZeroCopyRingBufferReader, ZeroCopyRingBufferReaderError,
};

pub struct DemoZeroCopyRingBufferReader {
    demo_data: Vec<u8>,
    read_pointer: usize,
    loaded_pointer: usize,
}

impl DemoZeroCopyRingBufferReader {
    pub fn new(demo_data: Vec<u8>) -> DemoZeroCopyRingBufferReader {
        DemoZeroCopyRingBufferReader {
            demo_data,
            read_pointer: 0,
            loaded_pointer: 0,
        }
    }

    // Helper method to check available bytes in source
    fn available_in_source(&self) -> usize {
        self.demo_data.len() - self.loaded_pointer
    }

    // Helper method to check available bytes in buffer
    fn available_in_buffer(&self) -> usize {
        self.loaded_pointer - self.read_pointer
    }
}

impl ZeroCopyRingBufferReader for DemoZeroCopyRingBufferReader {
    unsafe fn unsafe_data(&self) -> &[u8] {
        &self.demo_data[self.read_pointer..self.loaded_pointer]
    }

    fn load_data(&mut self, num_bytes: usize) -> Result<usize, ZeroCopyRingBufferReaderError> {
        // Calculate how many bytes we can actually load
        let can_load = min(num_bytes, self.available_in_source());

        // Update the loaded pointer
        self.loaded_pointer += can_load;

        Ok(can_load)
    }

    fn load_all_data(&mut self) -> Result<usize, ZeroCopyRingBufferReaderError> {
        // Load all remaining data from the source
        let available = self.available_in_source();

        // Update the loaded pointer to include all data
        self.loaded_pointer = self.demo_data.len();

        Ok(available)
    }

    fn discard_data(&mut self, num_bytes: usize) -> Result<usize, ZeroCopyRingBufferReaderError> {
        // Calculate how many bytes we can actually discard
        let can_discard = min(num_bytes, self.available_in_buffer());

        // Update the read pointer
        self.read_pointer += can_discard;

        Ok(can_discard)
    }

    fn discard_all_data(&mut self) -> Result<usize, ZeroCopyRingBufferReaderError> {
        // Discard all data in the buffer
        let available = self.available_in_buffer();

        // Move read pointer to catch up with loaded pointer
        self.read_pointer = self.loaded_pointer;

        Ok(available)
    }
}