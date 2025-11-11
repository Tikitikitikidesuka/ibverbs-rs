use crate::aliased_buffer::MockAliasedBufferWriter;
use crate::aliased_readable::VALID_MAGIC;
use crate::dynamic_size_element::{BufferedDiaryEntry, DiaryEntry, MockWritable, OwnedDiaryEntry};
use circular_buffer::{CircularBufferWritable, CircularBufferWriter};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WriteError {
    #[error("Not enough space for requested type")]
    NotEnoughSpace,
}

fn write_diary_entry<T: DiaryEntry + MockWritable>(
    diary_entry: &T,
    writer: &mut MockAliasedBufferWriter,
) -> Result<(), WriteError> {
    let aligned_size =
        utils::align_up_pow2(diary_entry.buffered_size(), writer.alignment_pow2());

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
    type WriteResult = Result<(), WriteError>;

    fn write(&self, writer: &mut MockAliasedBufferWriter) -> Self::WriteResult {
        write_diary_entry(self, writer)
    }
}

impl CircularBufferWritable<MockAliasedBufferWriter> for OwnedDiaryEntry {
    type WriteResult = Result<(), WriteError>;

    fn write(&self, writer: &mut MockAliasedBufferWriter) -> Self::WriteResult {
        write_diary_entry(self, writer)
    }
}
