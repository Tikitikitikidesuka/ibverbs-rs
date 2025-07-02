use crate::circular_buffer::CircularBufferReader;
use crate::multi_fragment_packet::MultiFragmentPacketRef;
use crate::pcie40::reader::PCIe40Reader;
use crate::pcie40::stream::stream::PCIe40StreamError;
use crate::typed_circular_buffer::{CircularBufferMultiReadable, CircularBufferReadable};
use crate::typed_circular_buffer_read_guard::{MultiReadGuard, ReadGuard};
use crate::utils;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PCIe40TypedReadError {
    #[error("Type not found on buffer")]
    NotFound,

    #[error("Not enough data for requested type")]
    NotEnoughData,

    #[error("Data is corrupt for requested type")]
    CorruptData,

    #[error("Unable to communicate with the stream: {0:?}")]
    StreamError(#[from] PCIe40StreamError),
}

impl<'r> CircularBufferReadable<PCIe40Reader<'r>> for MultiFragmentPacketRef {
    type ReadResult<'a> = Result<ReadGuard<'a, PCIe40Reader<'r>, Self>, PCIe40TypedReadError> where Self: 'a, PCIe40Reader<'r>: 'a;

    fn read<'a>(reader: &'a mut PCIe40Reader<'r>) -> Self::ReadResult<'a> {
        let readable_region = reader.readable_region()?;

        // Verify enough data for header
        if readable_region.len() < Self::HEADER_SIZE {
            return Err(PCIe40TypedReadError::NotEnoughData);
        }

        // Cast to mfp
        let mfp_mem =
            unsafe { &*(readable_region[..size_of::<Self>()].as_ptr() as *const Self) };

        // Verify valid magic packet
        if mfp_mem.magic() != Self::VALID_MAGIC {
            return Err(PCIe40TypedReadError::CorruptData);
        }

        // Verify enough data for the whole entry and alignment
        let aligned_size = utils::align_up_pow2(mfp_mem.packet_size() as usize, reader.alignment_pow2());
        if readable_region.len() < aligned_size {
            return Err(PCIe40TypedReadError::NotEnoughData);
        }

        // If all checks are passed, guard the type
        let read_guard = ReadGuard::new(reader, mfp_mem, aligned_size);

        Ok(read_guard)
    }
}

impl<'r> CircularBufferMultiReadable<PCIe40Reader<'r>> for MultiFragmentPacketRef {
    type MultiReadResult<'a> =
    Result<MultiReadGuard<'a, PCIe40Reader<'r>, Self>, PCIe40TypedReadError> where Self: 'a, PCIe40Reader<'r>: 'a;

    fn read_multiple<'a>(
        reader: &'a mut PCIe40Reader<'r>,
        num: usize,
    ) -> Self::MultiReadResult<'a> {
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
                &*(readable_region[advance_size..advance_size + size_of::<Self>()].as_ptr()
                    as *const Self)
            };

            // Verify valid magic packet
            if mfp_mem.magic() != Self::VALID_MAGIC {
                return Err(PCIe40TypedReadError::CorruptData);
            }

            // Verify enough data for the whole entry and alignment
            let aligned_size = utils::align_up_pow2(mfp_mem.packet_size() as usize, reader.alignment_pow2());
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
