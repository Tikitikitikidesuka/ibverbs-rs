use crate::MultiFragmentPacket;
use circular_buffer::{
    CircularBufferMultiReadable, CircularBufferReadable, CircularBufferReader, MultiReadGuard,
    ReadGuard,
};
use pcie40::reader::PCIe40Reader;
use pcie40::stream::stream::PCIe40StreamError;
use thiserror::Error;
use tracing::error;

/// Errors that can occur when reading an MFP from the PCIe40 card.
#[derive(Debug, Error)]
pub enum PCIe40TypedReadError {
    /// No data is yet present in the buffer.
    #[error("Type not found on buffer")]
    NotFound,

    /// Not enough data is yet present it the buffer to satisfy the request.
    #[error("Not enough data for requested type")]
    NotEnoughData,

    /// Corrupted data was returned, i.e. detected by an invalid magic number.
    #[error("Data is corrupt for requested type")]
    CorruptData,

    /// Error when communicating with the PICe40 stream.
    #[error("Unable to communicate with the stream: {0:?}")]
    StreamError(#[from] PCIe40StreamError),
}

impl<'r> CircularBufferReadable<PCIe40Reader> for MultiFragmentPacket {
    type ReadResult<'a>
        = Result<ReadGuard<'a, PCIe40Reader, Self>, PCIe40TypedReadError>
    where
        Self: 'a,
        PCIe40Reader: 'a;

    fn read<'a>(reader: &'a mut PCIe40Reader) -> Self::ReadResult<'a> {
        let readable_region = reader.readable_region()?;

        // Verify enough data for header
        if readable_region.len() < Self::HEADER_SIZE {
            return Err(PCIe40TypedReadError::NotEnoughData);
        }

        // Cast to mfp
        let mfp_mem = unsafe { &*(readable_region[..size_of::<Self>()].as_ptr() as *const Self) };

        // Verify valid magic packet
        if mfp_mem.magic() != Self::VALID_MAGIC {
            return Err(PCIe40TypedReadError::CorruptData);
        }

        // Verify enough data for the whole entry and alignment
        let aligned_size =
            ebutils::align_up_pow2(mfp_mem.packet_size() as usize, reader.alignment_pow2());
        if readable_region.len() < aligned_size {
            return Err(PCIe40TypedReadError::NotEnoughData);
        }

        // If all checks are passed, guard the type
        let read_guard = ReadGuard::new(reader, mfp_mem, aligned_size);

        Ok(read_guard)
    }
}

impl<'r> CircularBufferMultiReadable<PCIe40Reader> for MultiFragmentPacket {
    type MultiReadResult<'a>
        = Result<MultiReadGuard<'a, PCIe40Reader, Self>, PCIe40TypedReadError>
    where
        Self: 'a;

    fn read_multiple<'a>(reader: &'a mut PCIe40Reader, num: usize) -> Self::MultiReadResult<'a> {
        let readable_region = reader.readable_region()?;

        let mut advance_size = 0;
        let mut read_data = Vec::with_capacity(num);

        for _ in 0..num {
            // Verify enough data for header
            if readable_region.len() < Self::HEADER_SIZE + advance_size {
                return Err(PCIe40TypedReadError::NotEnoughData);
            }

            // Cast to mfp
            let mfp_mem = unsafe {
                &*(readable_region[advance_size..advance_size + size_of::<Self>()].as_ptr() // ! Is this correct? size_of::<MFP>()  is just the header, or undefined if unsized later on.
                    as *const Self)
            };

            // Verify valid magic packet
            if mfp_mem.magic() != Self::VALID_MAGIC {
                return Err(PCIe40TypedReadError::CorruptData);
            }

            // Verify enough data for the whole entry and alignment
            let aligned_size =
                ebutils::align_up_pow2(mfp_mem.packet_size() as usize, reader.alignment_pow2());
            if readable_region.len() < aligned_size + advance_size {
                return Err(PCIe40TypedReadError::NotEnoughData);
            }

            // Store reference to read entry and add advance size
            read_data.push(mfp_mem);

            advance_size += aligned_size;
        }

        // If all checks are passed, guard the type
        let read_guard = MultiReadGuard::new(reader, read_data, advance_size);

        Ok(read_guard)
    }
}
