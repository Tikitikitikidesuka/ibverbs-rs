use crate::circular_buffer::CircularBufferReader;
use crate::pcie40::stream::mapped_stream::PCIe40MappedStream;
use crate::pcie40::stream::stream::PCIe40StreamError;
use crate::utils;
use thiserror::Error;

pub struct PCIe40Reader<'a> {
    mapped_buffer: PCIe40MappedStream<'a>,
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

impl<'a> PCIe40Reader<'a> {
    pub fn new(
        mapped_buffer: PCIe40MappedStream<'a>,
        alignment_pow2: u8,
    ) -> Result<Self, PCIe40ReaderInstanceError> {
        // Size has to be aligned for advance alignment check to work properly
        let buffer_size = unsafe { mapped_buffer.data().len() };
        if !utils::check_alignment_pow2(buffer_size, alignment_pow2) {
            return Err(PCIe40ReaderInstanceError::BufferSizeNotAligned {
                buffer_size,
                alignment: utils::pow2(alignment_pow2),
            });
        }

        // Read offset has to be aligned for advance alignment check to work properly
        let read_offset = mapped_buffer.get_read_offset()?;
        if !utils::check_alignment_pow2(read_offset, alignment_pow2) {
            return Err(PCIe40ReaderInstanceError::ReadOffsetNotAligned {
                read_offset,
                alignment: utils::pow2(alignment_pow2),
            });
        }

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

impl<'r> CircularBufferReader for PCIe40Reader<'r> {
    type AdvanceResult = Result<(), PCIe40AdvanceError>;
    type ReadableRegionResult<'a> = Result<&'a [u8], PCIe40StreamError> where Self: 'a, 'r: 'a;

    fn advance_read_pointer(&mut self, bytes: usize) -> Self::AdvanceResult {
        if !utils::check_alignment_pow2(self.read_offset + bytes, self.alignment_pow2) {
            return Err(PCIe40AdvanceError::NotAligned);
        }

        // Move the read offset on the card and return if error
        self.mapped_buffer
            .move_read_offset(bytes)
            .map_err(|_| PCIe40AdvanceError::OutOfBounds)?;

        // Update local read offset wrapping to fit the buffer boundary
        self.read_offset = utils::wrap_around_pow2(self.read_offset + bytes, self.alignment_pow2);

        Ok(())
    }

    fn readable_region(&self) -> Self::ReadableRegionResult<'_> {
        let available_bytes = self.mapped_buffer.available_bytes()?;
        Ok(&unsafe { self.mapped_buffer.data() }[self.read_offset..(self.read_offset + available_bytes)])
    }
}
