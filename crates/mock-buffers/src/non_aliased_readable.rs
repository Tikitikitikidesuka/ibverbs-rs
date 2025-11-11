use crate::dynamic_size_element::{BufferedDiaryEntry, DiaryEntry};
use crate::non_aliased_buffer::MockNonAliasedBufferReader;
use circular_buffer::{
    CircularBufferMultiReadable, CircularBufferReadable, CircularBufferReader, MultiReadGuard,
    ReadGuard,
};
use thiserror::Error;

pub const VALID_MAGIC: [u8; 2] = [0xAA, 0xAA];
pub const WRAP_MAGIC: [u8; 2] = [0x55, 0x55];

#[derive(Debug, Error)]
pub enum ReadError {
    #[error("Type not found on buffer")]
    NotFound,

    #[error("Not enough data for requested type")]
    NotEnoughData,

    #[error("Data is corrupt for requested type")]
    CorruptData,
}

impl CircularBufferReadable<MockNonAliasedBufferReader> for BufferedDiaryEntry {
    type ReadResult<'a> = Result<ReadGuard<'a, MockNonAliasedBufferReader, Self>, ReadError>;

    fn read(reader: &mut MockNonAliasedBufferReader) -> Self::ReadResult<'_> {
        let (primary_region, secondary_region) = reader.readable_region();

        // Check if we have enough data to read the wrap flag
        if primary_region.len() < Self::magic_bytes_size() {
            return Err(ReadError::NotEnoughData);
        }

        // Determine which region to read from based on wrap flag
        let readable_region =
            if unsafe { Self::magic_bytes(primary_region.as_ptr()) } == &WRAP_MAGIC {
                secondary_region
            } else {
                primary_region
            };

        // Validate minimum size for header
        if readable_region.len() < size_of::<Self>() {
            return Err(ReadError::NotEnoughData);
        }

        // Cast to diary entry and validate magic
        let diary_entry =
            unsafe { &*(readable_region[..size_of::<Self>()].as_ptr() as *const Self) };

        if diary_entry.magic != VALID_MAGIC {
            return Err(ReadError::CorruptData);
        }

        // Calculate total size and validate space
        let total_length = size_of::<Self>() + diary_entry.note().len();
        let aligned_size = utils::align_up_pow2(total_length, reader.alignment_pow2());

        if readable_region.len() < aligned_size {
            return Err(ReadError::NotEnoughData);
        }

        Ok(ReadGuard::new(reader, diary_entry, aligned_size))
    }
}

impl CircularBufferMultiReadable<MockNonAliasedBufferReader> for BufferedDiaryEntry {
    type MultiReadResult<'a> =
        Result<MultiReadGuard<'a, MockNonAliasedBufferReader, Self>, ReadError>;

    fn read_multiple(
        reader: &mut MockNonAliasedBufferReader,
        num: usize,
    ) -> Self::MultiReadResult<'_> {
        let (primary_region, secondary_region) = reader.readable_region();
        let mut read_data = Vec::with_capacity(num);
        let mut advance_size = 0;
        let mut wrapped = false;

        for _ in 0..num {
            // Determine current reading position and region
            let (current_region, offset) = if !wrapped {
                // Check for wrap marker at current position
                if advance_size + Self::magic_bytes_size() > primary_region.len() {
                    return Err(ReadError::NotEnoughData);
                }

                let is_wrap = unsafe {
                    Self::magic_bytes(primary_region.as_ptr().add(advance_size)) == &WRAP_MAGIC
                };

                if is_wrap {
                    wrapped = true;
                    advance_size = primary_region.len();
                    (secondary_region, 0)
                } else {
                    (primary_region, advance_size)
                }
            } else {
                // Already wrapped, continue in secondary region
                let offset = advance_size - primary_region.len();
                (secondary_region, offset)
            };

            // Validate space for header
            if current_region.len() < size_of::<Self>() + offset {
                return Err(ReadError::NotEnoughData);
            }

            // Cast to diary entry and validate magic
            let diary_entry = unsafe {
                &*(current_region[offset..offset + size_of::<Self>()].as_ptr() as *const Self)
            };

            if diary_entry.magic != VALID_MAGIC {
                return Err(ReadError::CorruptData);
            }

            // Calculate entry size and validate total space
            let total_length = size_of::<Self>() + diary_entry.note().len();
            let aligned_entry_size =
                utils::align_up_pow2(total_length, reader.alignment_pow2());

            if current_region.len() < aligned_entry_size + offset {
                return Err(ReadError::NotEnoughData);
            }

            // Store entry and advance position
            read_data.push(diary_entry);
            advance_size += aligned_entry_size;
        }

        Ok(MultiReadGuard::new(reader, read_data, advance_size))
    }
}
