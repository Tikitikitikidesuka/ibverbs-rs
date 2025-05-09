use log::{debug, trace};
use pcie40_rs::zero_copy_ring_buffer_reader::{
    ZeroCopyRingBufferReader, ZeroCopyRingBufferReaderError,
};
use std::cmp::min;

pub struct ExampleReader {
    demo_data: Vec<u8>,
    read_pointer: usize,
    write_pointer: usize,
    demo_write_offset: usize,
    alignment: Option<usize>,
}

impl ExampleReader {
    pub fn new(demo_data: Vec<u8>, demo_write_offset: usize) -> ExampleReader {
        debug!(
            "Creating new ExampleReader with write offset: {}",
            demo_write_offset
        );
        ExampleReader {
            demo_data,
            read_pointer: 0,
            write_pointer: 0,
            demo_write_offset,
            alignment: None,
        }
    }

    pub fn with_alignment(
        demo_data: Vec<u8>,
        demo_write_offset: usize,
        alignment: usize,
    ) -> ExampleReader {
        let mut reader = Self::new(demo_data, demo_write_offset);
        reader.alignment = Some(alignment);
        reader
    }

    // Helper method to check available bytes in source
    fn available_in_source(&self) -> usize {
        let write_pointer = self.read_pointer + self.demo_write_offset;

        // Only return data up to the simulated write pointer
        // or the end of the demo data, whichever is smaller
        let available = min(self.demo_data.len(), write_pointer) - self.write_pointer;
        trace!(
            "Available in source: {} bytes (write_pointer: {}, loaded_pointer: {})",
            available, write_pointer, self.write_pointer
        );
        available
    }

    // Helper method to check available bytes in buffer
    fn available_in_buffer(&self) -> usize {
        let available = self.write_pointer - self.read_pointer;
        trace!(
            "Available in buffer: {} bytes (loaded_pointer: {}, read_pointer: {})",
            available, self.write_pointer, self.read_pointer
        );
        available
    }

    // Get the current simulated write pointer position
    pub fn write_pointer(&self) -> usize {
        let write_ptr = min(
            self.read_pointer + self.demo_write_offset,
            self.demo_data.len(),
        );
        trace!("Current write pointer: {}", write_ptr);
        write_ptr
    }
}

impl ZeroCopyRingBufferReader for ExampleReader {
    unsafe fn unsafe_data(&self) -> &[u8] {
        trace!(
            "Accessing data with read pointer {} and loaded pointer {}",
            self.read_pointer, self.write_pointer
        );
        &self.demo_data[self.read_pointer..self.write_pointer]
    }

    fn load_all_data(&mut self) -> Result<usize, ZeroCopyRingBufferReaderError> {
        debug!("Loading all available data");

        // Load all remaining data from the source, up to the write pointer
        let available = self.available_in_source();

        // Update the loaded pointer
        self.write_pointer += available;

        debug!(
            "Loaded {} bytes, new loaded pointer: {}",
            available, self.write_pointer
        );

        Ok(available)
    }

    fn discard_data(&mut self, num_bytes: usize) -> Result<usize, ZeroCopyRingBufferReaderError> {
        debug!("Discarding {} bytes of data", num_bytes);

        // Calculate how many bytes we can actually discard
        let can_discard = min(num_bytes, self.available_in_buffer());

        // Update the read pointer
        self.read_pointer += can_discard;

        debug!(
            "Discarded {} bytes, new read pointer: {}",
            can_discard, self.read_pointer
        );

        Ok(can_discard)
    }

    fn discard_all_data(&mut self) -> Result<usize, ZeroCopyRingBufferReaderError> {
        debug!("Discarding all buffered data");

        // Discard all data in the buffer
        let available = self.available_in_buffer();

        // Move read pointer to catch up with loaded pointer
        self.read_pointer = self.write_pointer;

        debug!(
            "Discarded {} bytes, read pointer now matches loaded pointer: {}",
            available, self.read_pointer
        );

        Ok(available)
    }

    fn alignment(&self) -> Result<Option<usize>, ZeroCopyRingBufferReaderError> {
        Ok(self.alignment)
    }
}

fn main() {}
