use crate::mock_buffers::ReadError;
use crate::mock_buffers::dynamic_size_element::{BufferedDiaryEntry, DiaryEntry};
use crate::mock_buffers::non_aliased::{MockNonAliasedBufferReader, VALID_MAGIC, WRAP_MAGIC};
use crate::{CircularBufferReadable, CircularBufferReader, SizedReadGuard};

pub type MockNonAliasedBufferReadGuard<'guard, T: ?Sized> =
    SizedReadGuard<'guard, MockNonAliasedBufferReader, T>;

impl<'guard, 'buf> CircularBufferReadable<'guard, 'buf, MockNonAliasedBufferReader>
    for BufferedDiaryEntry
where
    'buf: 'guard,
{
    type ReadGuard = MockNonAliasedBufferReadGuard<'guard, Self>;
    type ReadError = ReadError;

    fn read(
        reader: &'guard mut MockNonAliasedBufferReader,
        num: usize,
    ) -> Result<Self::ReadGuard, Self::ReadError> {
        let (primary_region, secondary_region) = reader.readable_region().unwrap();
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
            let aligned_entry_size = ebutils::align_up_pow2(total_length, reader.alignment_pow2());

            if current_region.len() < aligned_entry_size + offset {
                return Err(ReadError::NotEnoughData);
            }

            // Store entry and advance position
            read_data.push(diary_entry);
            advance_size += aligned_entry_size;
        }

        Ok(MockNonAliasedBufferReadGuard::from_reader(
            reader,
            read_data,
            advance_size,
        ))
    }
}
