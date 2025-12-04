use crate::stream::mapped_stream::PCIe40MappedStream;
use crate::stream::stream::PCIe40StreamError;
use circular_buffer::CircularBufferReader;
use thiserror::Error;
use tracing::{debug, instrument, warn};

pub struct PCIe40Reader {
    mapped_buffer: PCIe40MappedStream,
    read_offset: usize,
    alignment_pow2: u8,
}

#[derive(Debug, Error)]
pub enum PCIe40ReaderInstanceError {
    #[error(
        "Size of the buffer does not match alignment: Buffer size={buffer_size}, Alignment={alignment}"
    )]
    BufferSizeNotAligned {
        buffer_size: usize,
        alignment: usize,
    },

    #[error("Read offset is not aligned: Read offset={read_offset}, Alignment={alignment}")]
    ReadOffsetNotAligned {
        read_offset: usize,
        alignment: usize,
    },

    #[error("Unable to communicate with the stream: {0:?}")]
    StreamError(#[from] PCIe40StreamError),
}

#[derive(Debug, Error)]
pub enum PCIe40AdvanceError {
    #[error("Not enough data available")]
    OutOfBounds,
    #[error("Result address not aligned")]
    NotAligned,
}

impl PCIe40Reader {
    #[instrument(skip_all, fields(
        device_id = mapped_buffer.device_id(),
        alignment_pow2 = alignment_pow2
    ))]
    pub fn new(
        mapped_buffer: PCIe40MappedStream,
        alignment_pow2: u8,
    ) -> Result<Self, PCIe40ReaderInstanceError> {
        debug!("Creating PCIe40Reader instance");

        debug!("Checking buffer size fits alignment");
        let buffer_size = mapped_buffer.size();
        if !ebutils::check_alignment_pow2(buffer_size, alignment_pow2) {
            warn!("Buffer size does not match alignment");
            return Err(PCIe40ReaderInstanceError::BufferSizeNotAligned {
                buffer_size,
                alignment: ebutils::pow2(alignment_pow2),
            });
        }

        debug!("Checking read offset fits alignment");
        let read_offset = mapped_buffer.get_read_offset()?;
        if !ebutils::check_alignment_pow2(read_offset, alignment_pow2) {
            warn!("Read offset does not match alignment");
            return Err(PCIe40ReaderInstanceError::ReadOffsetNotAligned {
                read_offset,
                alignment: ebutils::pow2(alignment_pow2),
            });
        }

        debug!("PCIe40Reader instance created successfully");
        Ok(Self {
            mapped_buffer,
            read_offset,
            alignment_pow2,
        })
    }

    pub fn alignment_pow2(&self) -> u8 {
        self.alignment_pow2
    }
}

impl CircularBufferReader for PCIe40Reader {
    type AdvanceResult = Result<(), PCIe40AdvanceError>;
    type ReadableRegionResult<'a>
        = Result<&'a [u8], PCIe40StreamError>
    where
        Self: 'a;

    #[instrument(skip_all, fields(device_id = self.mapped_buffer.device_id(), bytes = bytes))]
    fn advance_read_pointer(&mut self, bytes: usize) -> Self::AdvanceResult {
        debug!("Attempting to advance the buffer's read pointer by {bytes} bytes");

        debug!("Checking buffer's alignment");
        if !ebutils::check_alignment_pow2(self.read_offset + bytes, self.alignment_pow2) {
            warn!("Aborting write pointer advance due to buffer's alignment violation");
            return Err(PCIe40AdvanceError::NotAligned);
        }

        // Move the read offset on the card and return if error
        debug!("Moving the read offset on the card");
        self.mapped_buffer.move_read_offset(bytes).map_err(|_| {
            warn!("Aborting write pointer due to insufficient buffer readable region space");
            PCIe40AdvanceError::OutOfBounds
        })?;

        debug!("All necessary checks for read pointer advance passed! Updating read pointer");
        debug!("Wrapping read offset to fit the buffer boundary (size/2) if necessary");
        self.read_offset =
            ebutils::wrap_around(self.read_offset + bytes, self.mapped_buffer.size() / 2);

        debug!("Read pointer advanced successfully");
        Ok(())
    }

    #[instrument(skip_all, fields(device_id = self.mapped_buffer.device_id()))]
    fn readable_region(&self) -> Self::ReadableRegionResult<'_> {
        debug!("Getting the buffer's readable region");
        let available_bytes = self.mapped_buffer.available_bytes()?;
        debug!("Available bytes: {available_bytes}");
        Ok(&unsafe { self.mapped_buffer.data() }
            [self.read_offset..(self.read_offset + available_bytes)])
    }
}
