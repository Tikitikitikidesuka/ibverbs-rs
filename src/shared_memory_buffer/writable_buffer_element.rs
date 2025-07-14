use std::any::type_name;
use std::fmt::Debug;
use crate::circular_buffer::CircularBufferWriter;
use crate::shared_memory_buffer::buffer_element::WritableSharedMemoryBufferElement;
use crate::shared_memory_buffer::reader::SharedMemoryBufferAdvanceError;
use crate::shared_memory_buffer::writer::SharedMemoryBufferWriter;
use crate::typed_circular_buffer::CircularBufferWritable;
use crate::utils;
use thiserror::Error;
use tracing::{debug, instrument, warn};

#[derive(Debug, Error)]
pub enum SharedMemoryTypedWriteError {
    #[error("Not enough data for requested type")]
    NotEnoughSpace,

    #[error("Unable to advance the write pointer: {0}")]
    AdvanceWritePointerError(#[from] SharedMemoryBufferAdvanceError),
}

/// Blanket implementation for all tyeps that implement `SharedMemoryBufferElement`
impl<T: WritableSharedMemoryBufferElement> CircularBufferWritable<SharedMemoryBufferWriter> for T {
    type WriteResult = Result<(), SharedMemoryTypedWriteError>;

    #[instrument(skip_all, fields(type = type_name::<T>(), shmem = writer.buffer_name()))]
    fn write(&self, writer: &mut SharedMemoryBufferWriter) -> Self::WriteResult {
        debug!("Attempting to write element to the buffer");

        debug!("Calculating the aligned size and getting the buffer's writable region");
        let aligned_size = utils::align_up_pow2(self.length_in_bytes(), writer.alignment_pow2());
        let (primary_region, secondary_region) = writer.writable_region();

        debug!("Determining which region of the buffer's writable region to write to");
        let (writable_region, advance_size) = if aligned_size <= primary_region.len() {
            debug!("The element's data fits on the primary region");
            (primary_region, aligned_size)
        } else {
            debug!("The element's data does not fit on the primary region");
            debug!("Setting the wrap flag on the primary region");
            Self::set_wrap_flag(primary_region).map_err(|error| {
                warn!("Unable to set the wrap flag on the primary region");
                error
            })?;

            debug!("Checking if there is enough space on the secondary region");
            if aligned_size > secondary_region.len() {
                warn!("The element's data does not fit on any of the two regions");
                return Err(SharedMemoryTypedWriteError::NotEnoughSpace);
            }

            debug!("The element's data fits on the secondary region");
            (secondary_region, aligned_size + primary_region.len())
        };

        debug!("Writing the element's data to the buffer");
        Self::write_to_buffer(self, writable_region).map_err(|error| {
            warn!("Unable to write element's data to the buffer");
            error
        })?;

        debug!("Advancing the write pointer to commit the write");
        writer.advance_write_pointer(advance_size).map_err(|error| {
            warn!("Unable to advance the write pointer. The write has not been committed");
            error
        })?;

        debug!("Wrote the element to the buffer successfully");

        Ok(())
    }
}
