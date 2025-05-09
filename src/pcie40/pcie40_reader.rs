use crate::pcie40::pcie40_stream::mapped_stream::PCIe40MappedStream;
use crate::pcie40::pcie40_stream::stream::PCIe40StreamError;
use crate::zero_copy_ring_buffer_reader::{
    ZeroCopyRingBufferReader, ZeroCopyRingBufferReaderError,
};
use log::{debug, error, trace};

pub struct PCIe40Reader<'a> {
    mapped_buffer: PCIe40MappedStream<'a>,
    read_offset: usize,
    write_offset: usize,
    alignment: usize,
}

impl<'a> PCIe40Reader<'a> {
    pub fn new(
        mapped_buffer: PCIe40MappedStream<'a>,
        alignment: usize,
    ) -> Result<Self, PCIe40StreamError> {
        debug!("Creating new PCIe40Reader");
        let read_offset = mapped_buffer.get_read_offset()?;
        let write_offset = mapped_buffer.get_write_offset()?;

        debug!("Initial read offset: {}", read_offset);

        Ok(Self {
            mapped_buffer,
            read_offset,
            write_offset,
            alignment,
        })
    }
}

impl ZeroCopyRingBufferReader for PCIe40Reader<'_> {
    unsafe fn unsafe_data(&self) -> &[u8] {
        trace!(
            "Accessing data with read offset {} and write offset {}",
            self.read_offset, self.write_offset
        );

        unsafe { &self.mapped_buffer.data()[self.read_offset..self.write_offset] }
    }

    fn load_all_data(&mut self) -> Result<usize, ZeroCopyRingBufferReaderError> {
        debug!("Loading all available data");

        let new_write_offset = self.mapped_buffer.get_write_offset().map_err(|error| {
            ZeroCopyRingBufferReaderError::ConnectionError(format!("{}", error))
        })?;

        let loaded_bytes = new_write_offset - self.write_offset;

        self.write_offset = new_write_offset;

        debug!(
            "Loaded {} bytes, new loaded data offset: {}",
            loaded_bytes, self.write_offset
        );

        Ok(loaded_bytes)
    }

    fn discard_data(&mut self, num_bytes: usize) -> Result<usize, ZeroCopyRingBufferReaderError> {
        debug!("Discarding {} bytes of data", num_bytes);

        let discarded_bytes = self.move_read_offset(num_bytes)?;

        debug!("Discarded {} bytes", discarded_bytes);

        Ok(discarded_bytes)
    }

    fn discard_all_data(&mut self) -> Result<usize, ZeroCopyRingBufferReaderError> {
        debug!("Discarding all loaded data");

        let available_bytes = self.write_offset - self.read_offset;

        let discarded_bytes = self.move_read_offset(available_bytes)?;

        debug!("Discarded {} bytes", discarded_bytes);

        Ok(discarded_bytes)
    }

    fn alignment(&self) -> Result<Option<usize>, ZeroCopyRingBufferReaderError> {
        Ok(Some(self.alignment))
    }
}

impl PCIe40Reader<'_> {
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
        self.read_offset += discarded_bytes;
        trace!("Read offset after update: {}", self.read_offset);

        debug!(
            "Successfully moved read offset by {} bytes",
            discarded_bytes
        );

        Ok(discarded_bytes)
    }
}
