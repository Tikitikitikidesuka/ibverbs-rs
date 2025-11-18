use crate::mock_buffers::WriteError;
use crate::mock_buffers::aliased::{MockAliasedBufferWriter, VALID_MAGIC};
use crate::mock_buffers::dynamic_size_element::{
    BufferedDiaryEntry, DiaryEntry, MockWritable, OwnedDiaryEntry,
};
use crate::{CircularBufferWritable, CircularBufferWriter};

/// Write implementation for diary entries to an aliased circular buffer.
///
/// This function writes diary entries into the buffer using a hybrid approach: the fixed-size
/// header is constructed in-place by casting the writable region to a mutable struct reference,
/// while the variable-length note content is copied from the source entry into the buffer's
/// body region.
///
/// # Write Process
///
/// 1. **Size calculation**: Computes aligned size needed for header + variable-length note
/// 2. **Space validation**: Ensures sufficient writable space exists
/// 3. **In-place header construction**: Casts writable bytes to `&mut BufferedDiaryEntry`
/// 4. **Header population**: Fills day, month, year, note length, and magic fields directly
/// 5. **Note copy**: Copies note string bytes into the body region following the header
/// 6. **Pointer advance**: Commits the write by advancing pointer by aligned size
///
/// Unlike read operations which return guards for deferred consumption, writes are committed
/// immediately upon successful completion. The write pointer advancement happens atomically
/// within this function.
fn write_diary_entry<T: DiaryEntry + MockWritable>(
    diary_entry: &T,
    writer: &mut MockAliasedBufferWriter,
) -> Result<(), WriteError> {
    let aligned_size = ebutils::align_up_pow2(diary_entry.buffered_size(), writer.alignment_pow2());

    let writable_region = writer.writable_region();

    // Validate enough space for write
    if aligned_size > writable_region.len() {
        return Err(WriteError::NotEnoughSpace);
    }

    // Cast writable memory to mutable ReadableDiaryEntryMem
    let typed_memory = unsafe { &mut *(writable_region.as_ptr() as *mut BufferedDiaryEntry) };

    // Fill in header data
    typed_memory.day = diary_entry.day();
    typed_memory.month = diary_entry.month();
    typed_memory.year = diary_entry.year();
    typed_memory.note_length = diary_entry.note().as_bytes().len() as u32;

    // Fill in magic packets
    typed_memory.magic.copy_from_slice(&VALID_MAGIC);

    // Fill in note in body
    let note_bytes = diary_entry.note().as_bytes();
    writable_region[size_of::<BufferedDiaryEntry>()..][..note_bytes.len()]
        .copy_from_slice(note_bytes);

    // Advance right pointer
    writer
        .advance_write_pointer(aligned_size)
        .map_err(|_| WriteError::NotEnoughSpace)?;

    Ok(())
}

impl CircularBufferWritable<MockAliasedBufferWriter> for BufferedDiaryEntry {
    type WriteError = WriteError;

    /// This implementation delegates to [`write_diary_entry()`] for zero-copy writing. It enables
    /// writing entries that are already in the buffered format directly into the circular buffer.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::NotEnoughSpace`] if insufficient space is available for the entry's
    /// aligned size.
    fn write(&self, writer: &mut MockAliasedBufferWriter) -> Result<(), Self::WriteError> {
        write_diary_entry(self, writer)
    }
}

impl CircularBufferWritable<MockAliasedBufferWriter> for OwnedDiaryEntry {
    type WriteError = WriteError;

    /// This implementation delegates to [`write_diary_entry()`] for zero-copy writing. It enables
    /// writing owned entries (with owned String notes) into the buffered format within the
    /// circular buffer. The entry's data is transformed into the wire format during the write.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::NotEnoughSpace`] if insufficient space is available for the entry's
    /// aligned size.
    fn write(&self, writer: &mut MockAliasedBufferWriter) -> Result<(), Self::WriteError> {
        write_diary_entry(self, writer)
    }
}
