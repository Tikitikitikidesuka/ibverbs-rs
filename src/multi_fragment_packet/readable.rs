use crate::multi_fragment_packet::{MultiFragmentPacketRef, MultiFragmentPacketFromRawBytesError, HEADER_SIZE};
use crate::typed_zero_copy_ring_buffer_reader::{
    ZeroCopyRingBufferReadable, ZeroCopyRingBufferReadableError, ensure_available_bytes,
};
use crate::utils;
use crate::zero_copy_ring_buffer_reader::ZeroCopyRingBufferReader;

impl<R> ZeroCopyRingBufferReadable<'_, R> for MultiFragmentPacketRef
where
    R: ZeroCopyRingBufferReader,
{
    fn load(reader: &mut R, offset: usize) -> Result<usize, ZeroCopyRingBufferReadableError> {
        // Ensure enough data for the header
        ensure_available_bytes(reader, offset + HEADER_SIZE)?;

        // Get temporary access to the data to read the header
        let temp_data = reader.data();
        let header_data = &temp_data[offset..(offset + HEADER_SIZE)];
        let mfp = unsafe { &*(header_data.as_ptr() as *const MultiFragmentPacketRef) };

        // Get the total packet size from the header
        let packet_size = mfp.packet_size() as usize;

        let (aligned_size, aligned_load) = if let Some(alignment) =
            reader.alignment().map_err(|error| {
                ZeroCopyRingBufferReadableError::ZeroCopyRingBufferReaderError(error)
            })? {
            (
                utils::align_up(packet_size, alignment),
                utils::align_up(offset + packet_size, alignment),
            )
        } else {
            (packet_size, offset + packet_size)
        };

        // Ensure enough data for the whole mfp
        ensure_available_bytes(reader, aligned_load)?;

        Ok(aligned_size)
    }

    fn cast(data: &[u8]) -> Result<&Self, ZeroCopyRingBufferReadableError> {
        MultiFragmentPacketRef::ref_from_raw_bytes(data).map_err(|error| match error {
            MultiFragmentPacketFromRawBytesError::NotEnoughDataAvailable {
                required_data: required_bytes,
                available_data: available_bytes,
            } => ZeroCopyRingBufferReadableError::NotEnoughDataAvailable {
                required_data: required_bytes,
                available_data: available_bytes,
            },
            MultiFragmentPacketFromRawBytesError::CorruptedMagic {
                read_magic,
                expected_magic,
            } => ZeroCopyRingBufferReadableError::ImproperlyFormattedData {
                message: format!(
                    "Expected magic bytes {:x?} but found {:x?}",
                    expected_magic, read_magic
                ),
            },
        })
    }
}
