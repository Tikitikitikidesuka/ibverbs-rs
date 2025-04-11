use crate::bindings::p40_stream_get_host_buf_bytes_used;
use crate::pcie40_stream::{PCIe40MappedBuffer, PCIe40StreamError};
use crate::utils;
use crate::zero_copy_ring_buffer_reader::{
    DataGuard, ZeroCopyRingBufferReader, ZeroCopyRingBufferReaderError,
};
use log::{debug, error, info, trace};

pub struct PCIe40Reader<'guard, 'buf> {
    mapped_buffer: PCIe40MappedBuffer<'guard, 'buf>,
    loaded_data_offset: usize,
    read_offset: usize,
    page_alignment_exp: u8,
}

impl<'guard, 'buf> PCIe40Reader<'guard, 'buf> {
    pub fn new(
        mapped_buffer: PCIe40MappedBuffer<'guard, 'buf>,
        page_alignment_exp: u8,
    ) -> Result<Self, PCIe40StreamError> {
        debug!("Creating new PCIe40Reader");
        let read_offset = mapped_buffer.get_read_offset()?;

        debug!("Initial read offset: {}", read_offset);

        Ok(Self {
            mapped_buffer,
            loaded_data_offset: read_offset,
            read_offset,
            page_alignment_exp,
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
        let aligned_num_bytes = utils::round_up_to_pow2_exp(num_bytes, self.page_alignment_exp);

        debug!(
            "Loading {} bytes of data (requesting {} bytes aligned to 2^{})",
            aligned_num_bytes, num_bytes, self.page_alignment_exp
        );

        let available_bytes = self.available_bytes()?;
        trace!("Total available bytes: {}", available_bytes);

        let aligned_available_bytes =
            utils::round_down_to_pow2_exp(available_bytes, self.page_alignment_exp);
        debug!(
            "Available aligned bytes: {} (aligned down to 2^{})",
            aligned_available_bytes, self.page_alignment_exp
        );

        let loaded_bytes = std::cmp::min(aligned_available_bytes, aligned_num_bytes);
        debug!(
            "Will load {} bytes (min of available aligned and requested aligned)",
            loaded_bytes
        );

        self.loaded_data_offset += loaded_bytes;

        debug!(
            "Loaded {} bytes, new loaded data offset: {} (previous: {})",
            loaded_bytes,
            self.loaded_data_offset,
            self.loaded_data_offset - loaded_bytes
        );

        Ok(loaded_bytes)
    }

    fn load_all_data(&mut self) -> Result<usize, ZeroCopyRingBufferReaderError> {
        debug!(
            "Loading all available data (with alignment 2^{})",
            self.page_alignment_exp
        );

        let available_bytes = self.available_bytes()?;
        trace!("Total available bytes: {}", available_bytes);

        let aligned_available_bytes =
            utils::round_down_to_pow2_exp(available_bytes, self.page_alignment_exp);
        debug!(
            "Available aligned bytes: {} (aligned down to 2^{})",
            aligned_available_bytes, self.page_alignment_exp
        );

        self.loaded_data_offset += aligned_available_bytes;

        debug!(
            "Loaded {} bytes, new loaded data offset: {} (previous: {})",
            aligned_available_bytes,
            self.loaded_data_offset,
            self.loaded_data_offset - aligned_available_bytes
        );

        Ok(aligned_available_bytes)
    }

    fn discard_data(&mut self, num_bytes: usize) -> Result<usize, ZeroCopyRingBufferReaderError> {
        let aligned_num_bytes = utils::round_up_to_pow2_exp(num_bytes, self.page_alignment_exp);

        debug!("Discarding {} bytes of data (requesting {} bytes aligned to 2^{})",
           aligned_num_bytes, num_bytes, self.page_alignment_exp);

        let available_bytes = self.available_bytes()?;
        trace!("Total available bytes: {}", available_bytes);

        let aligned_available_bytes = utils::round_down_to_pow2_exp(available_bytes, self.page_alignment_exp);
        trace!("Available aligned bytes: {}", aligned_available_bytes);

        let bytes_to_discard = std::cmp::min(aligned_available_bytes, aligned_num_bytes);
        debug!("Will discard {} bytes (min of available aligned and requested aligned)", bytes_to_discard);

        let discarded_bytes = self.move_read_offset(bytes_to_discard)?;

        debug!("Successfully discarded {} bytes", discarded_bytes);

        Ok(discarded_bytes)
    }

    fn discard_all_data(&mut self) -> Result<usize, ZeroCopyRingBufferReaderError> {
        debug!("Discarding all data (with alignment 2^{})", self.page_alignment_exp);

        let available_bytes = self.available_bytes()?;
        trace!("Total available bytes: {}", available_bytes);

        let aligned_available_bytes = utils::round_down_to_pow2_exp(available_bytes, self.page_alignment_exp);
        debug!("Will discard {} bytes (aligned down to 2^{})",
           aligned_available_bytes, self.page_alignment_exp);

        let discarded_bytes = self.move_read_offset(aligned_available_bytes)?;

        debug!("Successfully discarded {} bytes", discarded_bytes);

        Ok(discarded_bytes)
    }
}

impl<'guard, 'buf> PCIe40Reader<'guard, 'buf> {
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
