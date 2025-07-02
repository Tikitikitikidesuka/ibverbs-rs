use thiserror::Error;
use crate::circular_buffer::CircularBufferWriter;
use crate::shared_memory_buffer::buffer_element::SharedMemoryBufferElement;
use crate::shared_memory_buffer::writer::SharedMemoryBufferWriter;
use crate::typed_circular_buffer::CircularBufferWritable;
use crate::utils;

#[derive(Debug, Error)]
pub enum SharedMemoryTypedWriteError {
    #[error("Not enough data for requested type")]
    NotEnoughSpace,
}

impl<T: SharedMemoryBufferElement> CircularBufferWritable<SharedMemoryBufferWriter> for T {
    type WriteResult = Result<(), SharedMemoryTypedWriteError>;

    fn write(&self, writer: &mut SharedMemoryBufferWriter) -> Self::WriteResult {
        let aligned_size = utils::align_up_pow2(self.length_in_bytes(), writer.alignment_pow2());
        let (primary_region, secondary_region) = writer.writable_region();

        // Determine which region to write to and calculate advance size
        let (writable_region, advance_size) = if aligned_size <= primary_region.len() {
            // Fits in primary region
            (primary_region, aligned_size)
        } else {
            // Doesn't fit, write wrap marker and use secondary region
            Self::set_wrap_flag(primary_region);
            (secondary_region, aligned_size + primary_region.len())
        };

        // Check if we have enough space in the selected region
        if aligned_size > writable_region.len() {
            return Err(SharedMemoryTypedWriteError::NotEnoughSpace);
        }

        // Write the element data
        Self::cast_to_bytes(self, writable_region)?;

        // Commit the write
        writer
            .advance_write_pointer(advance_size)
            .map_err(|_| SharedMemoryTypedWriteError::NotEnoughSpace)
    }
}
