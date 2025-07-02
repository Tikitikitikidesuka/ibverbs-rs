use crate::multi_fragment_packet::{MultiFragmentPacket, MultiFragmentPacketRef};
use crate::shared_memory_buffer::buffer_element::{ReadableSharedMemoryBufferElement, SharedMemoryBufferElement, WritableSharedMemoryBufferElement};
use crate::shared_memory_buffer::readable_buffer_element::SharedMemoryTypedReadError;
use crate::shared_memory_buffer::writable_buffer_element::SharedMemoryTypedWriteError;

const WRAP_MAGIC: u16 = 0x5555;

impl SharedMemoryBufferElement for MultiFragmentPacketRef {
    fn length_in_bytes(&self) -> usize {
        self.packet_size() as usize
    }
}

impl ReadableSharedMemoryBufferElement for MultiFragmentPacketRef {
    fn cast_to_element(data: &[u8]) -> Result<&Self, SharedMemoryTypedReadError> {
        // Verify enough data for header
        if data.len() < Self::HEADER_SIZE {
            return Err(SharedMemoryTypedReadError::NotEnoughData);
        }

        // Cast to mfp
        let mfp = unsafe { &*(data[..size_of::<Self>()].as_ptr() as *const Self) };

        // Verify valid magic packet
        if mfp.magic() != Self::VALID_MAGIC {
            return Err(SharedMemoryTypedReadError::CorruptData);
        }

        Ok(&mfp)
    }

    fn check_wrap_flag(bytes: &[u8]) -> Result<bool, SharedMemoryTypedReadError> {
        // Check enough data for magic
        if bytes.len() < Self::magic_field_offset() + Self::magic_field_size() {
            return Err(SharedMemoryTypedReadError::NotEnoughData);
        }

        // If there is, cast mfp
        let mfp = unsafe { &*(bytes.as_ptr() as *const Self) };

        // And compare
        Ok(mfp.magic() == WRAP_MAGIC)
    }
}

impl WritableSharedMemoryBufferElement for MultiFragmentPacketRef {
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
        let mut magic =
            unsafe { &mut *(bytes[Self::magic_field_offset()..].as_mut_ptr() as *mut u16) };

        // And write it
        *magic = WRAP_MAGIC;

        Ok(())
    }
}

impl SharedMemoryBufferElement for MultiFragmentPacket {
    fn length_in_bytes(&self) -> usize {
        self.packet_size() as usize
    }
}

impl WritableSharedMemoryBufferElement for MultiFragmentPacket {
    fn write_to_buffer(&self, buffer: &mut [u8]) -> Result<(), SharedMemoryTypedWriteError> {
        // Write as if it were an MFPRef
        self.as_ref().write_to_buffer(buffer)
    }

    fn set_wrap_flag(bytes: &mut [u8]) -> Result<(), SharedMemoryTypedWriteError> {
        // Same as with the MFPRef
        MultiFragmentPacketRef::set_wrap_flag(bytes)
    }
}

/*
#[derive(Debug, Error)]
pub enum SharedMemoryTypedReadError {
    #[error("Type not found on buffer")]
    NotFound,

    #[error("Not enough data for requested type")]
    NotEnoughData,

    #[error("Data is corrupt for requested type")]
    CorruptData,
}

impl<'r> CircularBufferReadable<PCIe40Reader<'r>> for MultiFragmentPacketRef {
    type ReadResult<'a> = Result<ReadGuard<'a, PCIe40Reader<'r>, Self>, SharedMemoryTypedReadError> where Self: 'a, PCIe40Reader<'r>: 'a;

    fn read<'a>(reader: &'a mut PCIe40Reader<'r>) -> Self::ReadResult<'a> {
        let readable_region = reader.readable_region()?;

        // Verify enough data for header
        if readable_region.len() < Self::HEADER_SIZE {
            return Err(SharedMemoryTypedReadError::NotEnoughData);
        }

        // Cast to mfp
        let mfp_mem =
            unsafe { &*(readable_region[..size_of::<Self>()].as_ptr() as *const Self) };

        // Verify valid magic packet
        if mfp_mem.magic() != Self::VALID_MAGIC {
            return Err(SharedMemoryTypedReadError::CorruptData);
        }

        // Verify enough data for the whole entry and alignment
        let aligned_size = utils::align_up_pow2(mfp_mem.packet_size() as usize, reader.alignment_pow2());
        if readable_region.len() < aligned_size {
            return Err(SharedMemoryTypedReadError::NotEnoughData);
        }

        // If all checks are passed, guard the type
        let read_guard = ReadGuard::new(reader, mfp_mem, aligned_size);

        Ok(read_guard)
    }
}

impl<'r> CircularBufferMultiReadable<PCIe40Reader<'r>> for MultiFragmentPacketRef {
    type MultiReadResult<'a> =
    Result<MultiReadGuard<'a, PCIe40Reader<'r>, Self>, SharedMemoryTypedReadError> where Self: 'a, PCIe40Reader<'r>: 'a;

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
                return Err(SharedMemoryTypedReadError::NotEnoughData);
            }

            // Cast to mfp
            let mfp_mem = unsafe {
                &*(readable_region[advance_size..advance_size + size_of::<Self>()].as_ptr()
                    as *const Self)
            };

            // Verify valid magic packet
            if mfp_mem.magic() != Self::VALID_MAGIC {
                return Err(SharedMemoryTypedReadError::CorruptData);
            }

            // Verify enough data for the whole entry and alignment
            let aligned_size = utils::align_up_pow2(mfp_mem.packet_size() as usize, reader.alignment_pow2());
            if readable_region.len() < aligned_size + advance_size {
                return Err(SharedMemoryTypedReadError::NotEnoughData);
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
*/
