use thiserror::Error;
use crate::circular_buffer::CircularBufferWriter;
use crate::mock_buffers::dynamic_size_element::{BufferedDiaryEntry, DiaryEntry, MockWritable};
use crate::mock_buffers::non_aliased_buffer::MockNonAliasedBufferWriter;
use crate::mock_buffers::non_aliased_readable::{VALID_MAGIC, WRAP_MAGIC};
use crate::typed_circular_buffer::CircularBufferWritable;
use crate::utils;

#[derive(Debug, Error)]
pub enum WriteError {
    #[error("Not enough space for requested type")]
    NotEnoughSpace,
}

impl<T: MockWritable + DiaryEntry> CircularBufferWritable<MockNonAliasedBufferWriter> for T {
    type WriteResult = Result<(), WriteError>;

    fn write(&self, writer: &mut MockNonAliasedBufferWriter) -> Self::WriteResult {
        let aligned_size = utils::align_up_pow2(self.buffered_size(), writer.alignment_pow2());
        let (primary_region, secondary_region) = writer.writable_region();

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
        typed_memory.day = self.day();
        typed_memory.month = self.month();
        typed_memory.year = self.year();
        typed_memory.note_length = self.note().as_bytes().len() as u32;

        // Note content
        let note_bytes = self.note().as_bytes();
        let note_offset = size_of::<BufferedDiaryEntry>();
        writable_region[note_offset..note_offset + note_bytes.len()]
            .copy_from_slice(note_bytes);

        // Commit the write
        writer.advance_write_pointer(advance_size)
            .map_err(|_| WriteError::NotEnoughSpace)
    }
}
