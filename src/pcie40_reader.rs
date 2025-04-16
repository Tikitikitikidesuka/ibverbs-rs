use crate::pcie40_stream::{PCIe40MappedBuffer, PCIe40StreamError};
use crate::zero_copy_ring_buffer_reader::{
    ZeroCopyRingBufferReader, ZeroCopyRingBufferReaderError,
};
use log::{debug, error, trace};

pub struct PCIe40Reader<'guard, 'buf> {
    mapped_buffer: PCIe40MappedBuffer<'guard, 'buf>,
    loaded_data_offset: usize,
    read_offset: usize,
    alignment: usize,
}

impl<'guard, 'buf> PCIe40Reader<'guard, 'buf> {
    pub fn new(
        mapped_buffer: PCIe40MappedBuffer<'guard, 'buf>,
        alignment: usize,
    ) -> Result<Self, PCIe40StreamError> {
        debug!("Creating new PCIe40Reader");
        let read_offset = mapped_buffer.get_read_offset()?;

        debug!("Initial read offset: {}", read_offset);

        Ok(Self {
            mapped_buffer,
            loaded_data_offset: read_offset,
            read_offset,
            alignment,
        })
    }
}

impl ZeroCopyRingBufferReader for PCIe40Reader<'_, '_> {
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

    fn alignment(&mut self) -> Result<Option<usize>, ZeroCopyRingBufferReaderError> {
        Ok(Some(self.alignment))
    }
}

impl PCIe40Reader<'_, '_> {
    fn available_bytes(&self) -> Result<usize, ZeroCopyRingBufferReaderError> {
        trace!("Getting available bytes");

        let available = self.mapped_buffer.available_bytes().map_err(|error| {
            ZeroCopyRingBufferReaderError::ConnectionError(format!("{}", error))
        })?;

        Ok(available)
    }

    fn move_read_offset(
        &mut self,
        num_bytes: usize,
    ) -> Result<usize, ZeroCopyRingBufferReaderError> {
        debug!("Attempting to move read offset by {} bytes", num_bytes);

        let discarded_bytes = self
            .mapped_buffer
            .move_read_offset(num_bytes)
            .map_err(|error| {
                error!("Failed to move read offset: {}", error);
                ZeroCopyRingBufferReaderError::ConnectionError(format!("{}", error))
            })?;

        trace!("Read offset before update: {}", self.read_offset);
        self.read_offset = self.mapped_buffer.get_read_offset().map_err(|error| {
            ZeroCopyRingBufferReaderError::ConnectionError(format!("{}", error))
        })?;
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
