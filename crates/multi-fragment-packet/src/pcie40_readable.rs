use crate::{MultiFragmentPacket, MultiFragmentPacketFromRawBytesError};
use circular_buffer::{CircularBufferReadable, CircularBufferReader, ReadGuard};
use pcie40::reader::PCIe40Reader;
use pcie40::stream::stream::PCIe40StreamError;
use std::ops::Deref;
use thiserror::Error;

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

pub struct PCIe40ReadGuard<'guard, 'buf, T: ?Sized>
where
    'buf: 'guard,
{
    reader: &'guard mut PCIe40Reader<'buf>,
    read_data: Vec<&'guard T>,
    advance_size: usize,
}

impl<'guard, 'buf, T: ?Sized> Deref for PCIe40ReadGuard<'guard, 'buf, T>
where
    'buf: 'guard,
{
    type Target = [&'guard T];

    fn deref(&self) -> &Self::Target {
        self.read_data.as_slice()
    }
}

impl<'guard, 'buf, T: ?Sized> ReadGuard<'guard, PCIe40Reader<'buf>, T>
    for PCIe40ReadGuard<'guard, 'buf, T>
where
    'buf: 'guard,
{
    fn discard(self) -> <PCIe40Reader<'buf> as CircularBufferReader>::AdvanceResult
    where
        Self: Sized,
    {
        self.reader.advance_read_pointer(self.advance_size)
    }
}

impl<'guard, 'buf> CircularBufferReadable<'guard, 'buf, PCIe40Reader<'buf>> for MultiFragmentPacket
where
    'buf: 'guard,
{
    type ReadGuard
        = PCIe40ReadGuard<'guard, 'buf, Self>
    where
        Self: 'guard;
    type ReadError = PCIe40TypedReadError;

    fn read(
        reader: &'guard mut PCIe40Reader<'buf>,
        num: usize,
    ) -> Result<Self::ReadGuard, Self::ReadError> {
        let readable_region = reader.readable_region()?;

        let mut advance_size = 0;
        let mut read_data = Vec::with_capacity(num);

        for _ in 0..num {
            // Verify enough data for header
            if readable_region.len() < Self::HEADER_SIZE + advance_size {
                return Err(PCIe40TypedReadError::NotEnoughData);
            }

            // Cast to mfp
            // Decouple mfp reference from data lifetime but safe because
            // holding a reference to the reader
            let mfp = unsafe {
                &*(MultiFragmentPacket::from_raw_bytes(readable_region[advance_size..].as_ref())
                    .map_err(|error| match error {
                        MultiFragmentPacketFromRawBytesError::NotEnoughDataAvailable { .. } => {
                            PCIe40TypedReadError::NotEnoughData
                        }
                        MultiFragmentPacketFromRawBytesError::CorruptedMagic { .. } => {
                            PCIe40TypedReadError::CorruptData
                        }
                    })? as *const MultiFragmentPacket)
            };

            let aligned_size = ebutils::align_up_pow2(size_of_val(mfp), reader.alignment_pow2());

            // Store reference to read entry and add advance size
            read_data.push(mfp);
            advance_size += aligned_size;
        }

        // If all checks are passed, guard the type
        let read_guard = PCIe40ReadGuard {
            reader,
            advance_size,
            read_data,
        };

        Ok(read_guard)
    }
}
