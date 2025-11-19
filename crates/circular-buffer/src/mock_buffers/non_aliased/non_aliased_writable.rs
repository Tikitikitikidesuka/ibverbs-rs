use crate::mock_buffers::dynamic_size_element::{
    BufferedDiaryEntry, DiaryEntry, MockWritable, OwnedDiaryEntry,
};
use crate::{CircularBufferWritable, CircularBufferWriter};

use crate::mock_buffers::WriteError;
use crate::mock_buffers::non_aliased::{MockNonAliasedBufferWriter, VALID_MAGIC, WRAP_MAGIC};

impl CircularBufferWritable<MockNonAliasedBufferWriter> for BufferedDiaryEntry {
    type WriteStatus = ();
    type WriteError = WriteError;

    fn write(&self, writer: &mut MockNonAliasedBufferWriter) -> Result<(), Self::WriteError> {
        write_diary_entry(self, writer)
    }
}

impl CircularBufferWritable<MockNonAliasedBufferWriter> for OwnedDiaryEntry {
    type WriteStatus = ();
    type WriteError = WriteError;

    fn write(&self, writer: &mut MockNonAliasedBufferWriter) -> Result<(), Self::WriteError> {
        write_diary_entry(self, writer)
    }
}

fn write_diary_entry<T: DiaryEntry + MockWritable>(
    diary_entry: &T,
    writer: &mut MockNonAliasedBufferWriter,
) -> Result<(), WriteError> {
    let aligned_size = ebutils::align_up_pow2(diary_entry.buffered_size(), writer.alignment_pow2());
    let (primary_region, secondary_region) = writer.writable_region().unwrap();

    // Determine which region to write to and calculate advance size
    let (writable_region, advance_size) = if aligned_size <= primary_region.len() {
        // Fits in primary region
        (primary_region, aligned_size)
    } else {
        // Doesn't fit, write wrap marker and use secondary region
        unsafe { BufferedDiaryEntry::magic_bytes_mut(primary_region.as_mut_ptr()) }
            .copy_from_slice(&WRAP_MAGIC);
        (secondary_region, aligned_size + primary_region.len())
    };

    // Check if we have enough space in the selected region
    if aligned_size > writable_region.len() {
        return Err(WriteError::NotEnoughSpace);
    }

    // Write the diary entry data
    let typed_memory = unsafe { &mut *(writable_region.as_ptr() as *mut BufferedDiaryEntry) };

    // Header fields
    typed_memory.magic.copy_from_slice(&VALID_MAGIC);
    typed_memory.day = diary_entry.day();
    typed_memory.month = diary_entry.month();
    typed_memory.year = diary_entry.year();
    typed_memory.note_length = diary_entry.note().as_bytes().len() as u32;

    // Note content
    let note_bytes = diary_entry.note().as_bytes();
    let note_offset = size_of::<BufferedDiaryEntry>();
    writable_region[note_offset..note_offset + note_bytes.len()].copy_from_slice(note_bytes);

    // Commit the write
    writer
        .advance_write_pointer(advance_size)
        .map_err(|_| WriteError::NotEnoughSpace)
}
