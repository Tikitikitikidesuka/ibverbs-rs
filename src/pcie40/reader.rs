use crate::circular_buffer::CircularBufferReader;
use crate::pcie40::stream::mapped_stream::PCIe40MappedStream;
use crate::pcie40::stream::stream::PCIe40StreamError;
use crate::utils;
use thiserror::Error;

pub struct PCIe40Reader<'a> {
    mapped_buffer: PCIe40MappedStream<'a>,
    read_offset: usize,
    alignment_2pow: u8,
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

    #[error("Unable to communicate with the stream {stream_error:?}")]
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
    pub fn new(
        mapped_buffer: PCIe40MappedStream,
        alignment_2pow: u8,
    ) -> Result<Self, PCIe40ReaderInstanceError> {
        // Size has to be aligned for advance alignment check to work properly
        let buffer_size = unsafe { mapped_buffer.data().len() };
        if !utils::check_alignment_pow2(buffer_size, alignment_2pow) {
            return Err(PCIe40ReaderInstanceError::BufferSizeNotAligned {
                buffer_size,
                alignment: utils::pow2(alignment_2pow),
            });
        }

        // Read offset has to be aligned for advance alignment check to work properly
        let read_offset = mapped_buffer.get_read_offset()?;
        if !utils::check_alignment_pow2(read_offset, alignment_2pow) {
            return Err(PCIe40ReaderInstanceError::ReadOffsetNotAligned {
                read_offset,
                alignment: utils::pow2(alignment_2pow),
            });
        }

        Ok(Self {
            mapped_buffer,
            read_offset,
            alignment_2pow,
        })
    }
}

impl<'a> CircularBufferReader for PCIe40Reader<'a> {
    type AdvanceResult = Result<(), PCIe40AdvanceError>;
    type ReadableRegionResult = Result<&'a [u8], PCIe40StreamError>;

    fn advance_read_pointer(&mut self, bytes: usize) -> Self::AdvanceResult {
        if !utils::check_alignment_pow2(self.read_offset + bytes, self.alignment_2pow) {
            return Err(PCIe40AdvanceError::NotAligned);
        }

        // Move the read offset on the card and return if error
        self.mapped_buffer
            .move_read_offset(bytes)
            .map_err(|_| PCIe40AdvanceError::OutOfBounds)?;

        // Update local read offset wrapping to fit the buffer boundary
        self.read_offset = utils::wrap_around_pow2(self.read_offset + bytes, self.alignment_2pow);

        Ok(())
    }

    fn readable_region(&self) -> Self::ReadableRegionResult {
        let write_offset = self.mapped_buffer.get_write_offset()?;
        Ok(&unsafe { self.mapped_buffer.data() }[self.read_offset..write_offset])
    }
}
