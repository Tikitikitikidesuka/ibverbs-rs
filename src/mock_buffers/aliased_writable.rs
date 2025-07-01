use thiserror::Error;
use crate::circular_buffer::CircularBufferWriter;
use crate::mock_buffers::aliased_buffer::MockAliasedBufferWriter;
use crate::mock_buffers::aliased_readable::VALID_MAGIC;
use crate::mock_buffers::dynamic_size_element::{BufferedDiaryEntry, DiaryEntry, MockWritable};
use crate::typed_circular_buffer::CircularBufferWritable;
use crate::utils;

#[derive(Debug, Error)]
pub enum WriteError {
    #[error("Not enough space for requested type")]
    NotEnoughSpace,
}

impl<T: MockWritable + DiaryEntry> CircularBufferWritable<MockAliasedBufferWriter> for T {
    type WriteResult = Result<(), WriteError>;

    fn write(&self, writer: &mut MockAliasedBufferWriter) -> Self::WriteResult {
        let aligned_size = utils::align_up_pow2(self.buffered_size(), writer.alignment_pow2());

        let writable_region = writer.writable_region();

        // Validate enough space for write
        if aligned_size > writable_region.len() {
            return Err(WriteError::NotEnoughSpace);
        }

        // Cast writable memory to mutable ReadableDiaryEntryMem
        let typed_memory =
            unsafe { &mut *(writable_region.as_ptr() as *mut BufferedDiaryEntry) };

        // Fill in header data
        typed_memory.day = self.day();
        typed_memory.month = self.month();
        typed_memory.year = self.year();
        typed_memory.note_length = self.note().as_bytes().len() as u32;

        // Fill in magic packets
        typed_memory.magic.copy_from_slice(&VALID_MAGIC);

        // Fill in note in body
        let note_bytes = self.note().as_bytes();
        writable_region[size_of::<BufferedDiaryEntry>()..][..note_bytes.len()]
            .copy_from_slice(note_bytes);

        // Advance right pointer
        writer
            .advance_write_pointer(aligned_size)
            .map_err(|_| WriteError::NotEnoughSpace)?;

        Ok(())
    }
}