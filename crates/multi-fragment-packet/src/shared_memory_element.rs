use crate::{MultiFragmentPacket, MultiFragmentPacketFromRawBytesError, MultiFragmentPacketHeader, MultiFragmentPacketOwned};
use circular_buffer::{CircularBufferWritable, CircularBufferWriter};
use shared_memory_buffer::{ReadableSharedMemoryBufferElement, SharedMemoryBufferElement, SharedMemoryTypedReadError, SharedMemoryTypedWriteError, WritableSharedMemoryBufferElement, impl_circular_buffer_readable, impl_circular_buffer_writable, SharedMemoryBufferWriter};

const WRAP_MAGIC: u16 = 0xBF31;

impl_circular_buffer_writable!(MultiFragmentPacket);
impl_circular_buffer_readable!(MultiFragmentPacket);

impl SharedMemoryBufferElement for MultiFragmentPacket {
    fn length_in_bytes(&self) -> usize {
        self.packet_size() as usize
    }
}

impl ReadableSharedMemoryBufferElement for MultiFragmentPacket {
    fn cast_to_element(data: &[u8]) -> Result<&Self, SharedMemoryTypedReadError> {
        // Cast to mfp
        let mfp = MultiFragmentPacket::from_raw_bytes(data).map_err(|error| match error {
            MultiFragmentPacketFromRawBytesError::NotEnoughDataAvailable { .. } => {
                SharedMemoryTypedReadError::NotEnoughData
            }
            MultiFragmentPacketFromRawBytesError::CorruptedMagic { .. } => {
                SharedMemoryTypedReadError::CorruptData
            }
        })?;

        Ok(mfp)
    }

    fn check_wrap_flag(bytes: &[u8]) -> Result<bool, SharedMemoryTypedReadError> {
        // Check enough data for magic
        if bytes.len() < Self::magic_field_offset() + Self::magic_field_size() {
            return Err(SharedMemoryTypedReadError::NotEnoughData);
        }

        // If there is, cast header
        let mfp = unsafe { &*(bytes.as_ptr() as *const MultiFragmentPacketHeader) };

        // And compare
        Ok(mfp.magic == WRAP_MAGIC)
    }
}

impl WritableSharedMemoryBufferElement for MultiFragmentPacket {
    fn write_to_buffer(&self, buffer: &mut [u8]) -> Result<(), SharedMemoryTypedWriteError> {
        let mfp_slice = self.raw_packet_data();

        // Check enough space
        if buffer.len() < mfp_slice.len() {
            return Err(SharedMemoryTypedWriteError::NotEnoughSpace);
        }

        // Write mfp data
        buffer[..mfp_slice.len()].copy_from_slice(self.raw_packet_data());

        Ok(())
    }

    fn set_wrap_flag(bytes: &mut [u8]) -> Result<(), SharedMemoryTypedWriteError> {
        // Check enough data for magic
        if bytes.len() < Self::magic_field_offset() + Self::magic_field_size() {
            return Err(SharedMemoryTypedWriteError::NotEnoughSpace);
        }

        // If there is, cast magic
        let magic = unsafe { &mut *(bytes[Self::magic_field_offset()..].as_mut_ptr() as *mut u16) };

        // And write it
        *magic = WRAP_MAGIC;

        Ok(())
    }
}

impl SharedMemoryBufferElement for MultiFragmentPacketOwned {
    fn length_in_bytes(&self) -> usize {
        self.packet_size() as usize
    }
}

impl WritableSharedMemoryBufferElement for MultiFragmentPacketOwned {
    fn write_to_buffer(&self, buffer: &mut [u8]) -> Result<(), SharedMemoryTypedWriteError> {
        // Write as if it were an MFPRef
        self.as_ref().write_to_buffer(buffer)
    }

    fn set_wrap_flag(bytes: &mut [u8]) -> Result<(), SharedMemoryTypedWriteError> {
        // Same as with the MFPRef
        MultiFragmentPacket::set_wrap_flag(bytes)
    }
}
