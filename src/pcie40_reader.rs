use crate::pcie40_stream::{PCIe40MappedBuffer, PCIe40StreamError};
use crate::zero_copy_ring_buffer_reader::{
    DataGuard, ZeroCopyRingBufferReader, ZeroCopyRingBufferReaderError,
};
use log::{debug, error, info, trace};

pub struct PCIe40Reader<'guard, 'buf> {
    mapped_buffer: PCIe40MappedBuffer<'guard, 'buf>,
    loaded_data_offset: usize,
    read_offset: usize,
}

impl<'guard, 'buf> PCIe40Reader<'guard, 'buf> {
    pub fn new(mapped_buffer: PCIe40MappedBuffer<'guard, 'buf>) -> Result<Self, PCIe40StreamError> {
        debug!("Creating new PCIe40Reader");
        let read_offset = mapped_buffer.get_read_offset()?;

        debug!("Initial read offset: {}", read_offset);

        Ok(Self {
            mapped_buffer,
            loaded_data_offset: read_offset,
            read_offset,
        })
    }
}

impl<'guard, 'buf> ZeroCopyRingBufferReader for PCIe40Reader<'guard, 'buf> {
    unsafe fn unsafe_data(&self) -> &[u8] {
        trace!(
            "Accessing data with read offset {} and loaded data offset {}",
            self.read_offset, self.loaded_data_offset
        );

        unsafe { &self.mapped_buffer.data()[self.read_offset..self.loaded_data_offset] }
    }

    fn load_data(&mut self, num_bytes: usize) -> Result<usize, ZeroCopyRingBufferReaderError> {
        debug!("Loading {} bytes of data", num_bytes);

        let available_bytes = self.available_bytes()?;

        let loaded_bytes = std::cmp::min(available_bytes, num_bytes);

        self.loaded_data_offset += loaded_bytes;

        debug!(
            "Loaded {} bytes, new loaded data offset: {}",
            loaded_bytes, self.loaded_data_offset
        );

        Ok(loaded_bytes)
    }

    fn load_all_data(&mut self) -> Result<usize, ZeroCopyRingBufferReaderError> {
        debug!("Loading all available data");

        let available_bytes = self.available_bytes()?;

        self.loaded_data_offset += available_bytes;

        debug!(
            "Loaded {} bytes, new loaded data offset: {}",
            available_bytes, self.loaded_data_offset
        );

        Ok(available_bytes)
    }

    fn discard_data(&mut self, num_bytes: usize) -> Result<usize, ZeroCopyRingBufferReaderError> {
        debug!("Discarding {} bytes of data", num_bytes);

        let discarded_bytes = self.move_read_offset(num_bytes)?;

        debug!("Discarded {} bytes", discarded_bytes);

        Ok(discarded_bytes)
    }

    fn discard_all_data(&mut self) -> Result<usize, ZeroCopyRingBufferReaderError> {
        debug!("Discarding all data");

        let available_bytes = self.available_bytes()?;

        let discarded_bytes = self.move_read_offset(available_bytes)?;

        debug!("Discarded {} bytes", discarded_bytes);

        Ok(discarded_bytes)
    }
}

impl<'guard, 'buf> PCIe40Reader<'guard, 'buf> {
    fn available_bytes(&self) -> Result<usize, ZeroCopyRingBufferReaderError> {
        trace!("Calculating available bytes");

        let write_offset = self.mapped_buffer.get_write_offset().map_err(|error| {
            error!("Failed to get write offset: {}", error);
            ZeroCopyRingBufferReaderError::ConnectionError(format!("{}", error))
        })?;

        let read_offset = self.mapped_buffer.get_read_offset().map_err(|error| {
            error!("Failed to get read offset: {}", error);
            ZeroCopyRingBufferReaderError::ConnectionError(format!("{}", error))
        })?;

        let available = if write_offset < read_offset {
            0
        } else {
            write_offset - read_offset
        };
        trace!(
            "Available bytes: {} (write offset: {}, read offset: {})",
            available, write_offset, read_offset
        );

        Ok(available)
    }

    fn move_read_offset(
        &mut self,
        num_bytes: usize,
    ) -> Result<usize, ZeroCopyRingBufferReaderError> {
        trace!("Moving read offset by {} bytes", num_bytes);

        let write_offset = self.mapped_buffer.get_write_offset().map_err(|error| {
            error!("Failed to get write offset: {}", error);
            ZeroCopyRingBufferReaderError::ConnectionError(format!("{}", error))
        })?;

        let move_bytes = std::cmp::min(write_offset, num_bytes);
        debug!("Attempting to move read offset by {} bytes", move_bytes);

        let discarded_bytes = self
            .mapped_buffer
            .move_read_offset(move_bytes)
            .map_err(|error| {
                error!("Failed to move read offset: {}", error);
                ZeroCopyRingBufferReaderError::ConnectionError(format!("{}", error))
            })?;

        trace!("Read offset before update: {}", self.read_offset);
        self.read_offset += discarded_bytes;
        trace!("Read offset after update: {}", self.read_offset);

        self.loaded_data_offset = std::cmp::max(self.read_offset, self.loaded_data_offset);
        trace!("Updated loaded data offset: {}", self.loaded_data_offset);

        debug!(
            "Successfully moved read offset by {} bytes",
            discarded_bytes
        );

        Ok(discarded_bytes)
    }
}
