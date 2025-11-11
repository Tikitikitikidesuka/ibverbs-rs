use crate::aliased_buffer::MockAliasedBufferReader;
use crate::dynamic_size_element::{BufferedDiaryEntry, DiaryEntry};
use circular_buffer::{
    CircularBufferMultiReadable, CircularBufferReadable, CircularBufferReader, MultiReadGuard,
    ReadGuard,
};
use thiserror::Error;

pub const VALID_MAGIC: [u8; 2] = [0xAA, 0xAA];

#[derive(Debug, Error)]
pub enum ReadError {
    #[error("Type not found on buffer")]
    NotFound,

    #[error("Not enough data for requested type")]
    NotEnoughData,

    #[error("Data is corrupt for requested type")]
    CorruptData,
}

impl CircularBufferReadable<MockAliasedBufferReader> for BufferedDiaryEntry {
    type ReadResult<'a> = Result<ReadGuard<'a, MockAliasedBufferReader, Self>, ReadError>;

    fn read(reader: &mut MockAliasedBufferReader) -> Self::ReadResult<'_> {
        let readable_region = reader.readable_region();

        // Verify enough data for header
        if readable_region.len() < size_of::<Self>() {
            return Err(ReadError::NotEnoughData);
        }

        // Cast to header
        let diary_entry_mem =
            unsafe { &*(readable_region[..size_of::<Self>()].as_ptr() as *const Self) };

        // Verify valid magic packet
        if diary_entry_mem.magic != VALID_MAGIC {
            return Err(ReadError::CorruptData);
        }

        // Verify enough data for whole entry and alignment
        let total_length = size_of::<Self>() + diary_entry_mem.note().len();
        let aligned_size = ebutils::align_up_pow2(total_length, reader.alignment_pow2());
        if readable_region.len() < aligned_size {
            return Err(ReadError::NotEnoughData);
        }

        // If all checks are passed guard the type
        let read_guard = ReadGuard::new(reader, diary_entry_mem, aligned_size);

        Ok(read_guard)
    }
}

impl CircularBufferMultiReadable<MockAliasedBufferReader> for BufferedDiaryEntry {
    type MultiReadResult<'a> = Result<MultiReadGuard<'a, MockAliasedBufferReader, Self>, ReadError>;

    fn read_multiple(
        reader: &mut MockAliasedBufferReader,
        num: usize,
    ) -> Self::MultiReadResult<'_> {
        let readable_region = reader.readable_region();

        let mut advance_size = 0;
        let mut read_data = Vec::with_capacity(num);

        for _ in 0..num {
            // Verify enough data for header
            if readable_region.len() < size_of::<Self>() + advance_size {
                return Err(ReadError::NotEnoughData);
            }

            // Cast to header
            let diary_entry_mem = unsafe {
                &*(readable_region[advance_size..advance_size + size_of::<Self>()].as_ptr()
                    as *const Self)
            };

            // Verify valid magic packet
            if diary_entry_mem.magic != VALID_MAGIC {
                return Err(ReadError::CorruptData);
            }

            // Verify enough data for whole entry and alignment
            let total_length = size_of::<Self>() + diary_entry_mem.note().len();
            let aligned_size =
                ebutils::align_up_pow2(total_length, reader.alignment_pow2());
            if readable_region.len() < aligned_size + advance_size {
                return Err(ReadError::NotEnoughData);
            }

            // Store reference to read entry and add advance size
            read_data.push(diary_entry_mem);
            advance_size += aligned_size;
        }

        // If all checks are passed guard the type
        let read_guard = MultiReadGuard::new(reader, read_data, advance_size);

        Ok(read_guard)
    }
}
