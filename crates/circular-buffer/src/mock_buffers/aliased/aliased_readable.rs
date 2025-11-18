use crate::mock_buffers::ReadError;
use crate::mock_buffers::aliased::{MockAliasedBufferReader, VALID_MAGIC};
use crate::mock_buffers::dynamic_size_element::{BufferedDiaryEntry, DiaryEntry};
use crate::{CircularBufferReadable, CircularBufferReader, ReadGuard, SizedReadGuard};

pub type MockAliasedBufferReadGuard<'guard, T> = SizedReadGuard<'guard, MockAliasedBufferReader, T>;

impl<'guard, 'buf> CircularBufferReadable<'guard, 'buf, MockAliasedBufferReader>
    for BufferedDiaryEntry
where
    'buf: 'guard,
{
    type ReadGuard = MockAliasedBufferReadGuard<'guard, Self>;
    type ReadError = ReadError;

    /// This implementation extends single-entry reading to batch operations, validating and
    /// collecting references to multiple entries in one call. The batch read provides transactional
    /// semantics: either all `num` entries are successfully validated and returned, or an error
    /// occurs and no entries are returned.
    ///
    /// # Validation Process
    ///
    /// For each of the `num` requested entries, performs the same staged validation as single reads:
    ///
    /// 1. **Header size check**: Ensures header fits in remaining readable space after previous entries
    /// 2. **Magic number validation**: Verifies entry integrity
    /// 3. **Full entry size check**: Validates complete aligned entry fits in buffer
    /// 4. **Accumulation**: Tracks cumulative `advance_size` across all entries
    ///
    /// The validation loops through all entries sequentially, building up the total advance size
    /// needed to consume them all. This leverages the aliased buffer's guarantee of contiguous
    /// memory access, even across wrap boundaries.
    ///
    /// # Transactional Semantics
    ///
    /// If any entry fails validation, the entire operation fails and returns an error. In this case:
    /// - No [`MultiReadGuard`] is created
    /// - The read pointer remains at its original position
    /// - No entries are consumed from the buffer
    ///
    /// # Consumption Model
    ///
    /// The returned [`MultiReadGuard`] provides slice-like access to all entry references but
    /// **does not** advance the read pointer automatically. The caller must explicitly call
    /// [`MultiReadGuard::discard()`] to commit the batch read and advance by the cumulative
    /// aligned size of all entries.
    ///
    /// # Errors
    ///
    /// Returns [`ReadError::NotEnoughData`] if fewer than `num` complete entries are available,
    /// or if any individual entry's aligned size would exceed the remaining readable space.
    /// When this occurs, the read pointer remains unchanged.
    ///
    /// Returns [`ReadError::CorruptData`] if any entry fails magic number validation. No entries
    /// are consumed when corruption is detected.
    fn read(
        reader: &'guard mut MockAliasedBufferReader,
        num: usize,
    ) -> Result<Self::ReadGuard, Self::ReadError> {
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
            let aligned_size = ebutils::align_up_pow2(total_length, reader.alignment_pow2());
            if readable_region.len() < aligned_size + advance_size {
                return Err(ReadError::NotEnoughData);
            }

            // Store reference to read entry and add advance size
            read_data.push(diary_entry_mem);
            advance_size += aligned_size;
        }

        // If all checks are passed guard the type
        let read_guard = MockAliasedBufferReadGuard::from_reader(reader, read_data, advance_size);

        Ok(read_guard)
    }
}
