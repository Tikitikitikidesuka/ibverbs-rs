use crate::reader::SharedMemoryBufferAdvanceError;
use std::fmt::Debug;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SharedMemoryTypedWriteError {
    #[error("Not enough data for requested type")]
    NotEnoughSpace,

    #[error("Unable to advance the write pointer: {0}")]
    AdvanceWritePointerError(#[from] SharedMemoryBufferAdvanceError),
}

/// Macro to implement `CircularBufferWritable<SharedMemoryBufferWriter>` for types
/// that implement `WritableSharedMemoryBufferElement`.
///
/// This macro generates the implementation that would violate orphan rules if done
/// as a blanket implementation.
#[macro_export]
macro_rules! impl_circular_buffer_writable {
    ($type:ty) => {
        impl $crate::CircularBufferWritable<$crate::SharedMemoryBufferWriter> for $type {
            type WriteResult = Result<(), $crate::SharedMemoryTypedWriteError>;

            fn write(&self, writer: &mut $crate::SharedMemoryBufferWriter) -> Self::WriteResult {
                let aligned_size =
                    ebutils::align_up_pow2(self.length_in_bytes(), writer.alignment_pow2());
                let (primary_region, secondary_region) = writer.writable_region();

                let (writable_region, advance_size) = if aligned_size <= primary_region.len() {
                    (primary_region, aligned_size)
                } else {
                    Self::set_wrap_flag(primary_region)?;

                    if aligned_size > secondary_region.len() {
                        return Err($crate::SharedMemoryTypedWriteError::NotEnoughSpace);
                    }

                    (secondary_region, aligned_size + primary_region.len())
                };

                Self::write_to_buffer(self, writable_region)?;

                writer.advance_write_pointer(advance_size)?;

                Ok(())
            }
        }
    };
}

// Blanket implementation for all types that implement `SharedMemoryBufferElement`.
// Violates orphan rules so a macro for the user to call is offered instead.
/*
impl<T: WritableSharedMemoryBufferElement> CircularBufferWritable<SharedMemoryBufferWriter> for T {
    type WriteResult = Result<(), SharedMemoryTypedWriteError>;

    #[instrument(skip_all, fields(type = type_name::<T>(), shmem = writer.buffer_name()))]
    fn write(&self, writer: &mut SharedMemoryBufferWriter) -> Self::WriteResult {
        debug!("Attempting to write element to the buffer");

        debug!("Calculating the aligned size and getting the buffer's writable region");
        let aligned_size = ebutils::align_up_pow2(self.length_in_bytes(), writer.alignment_pow2());
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
        writer
            .advance_write_pointer(advance_size)
            .map_err(|error| {
                warn!("Unable to advance the write pointer. The write has not been committed");
                error
            })?;

        debug!("Wrote the element to the buffer successfully");

        Ok(())
    }
}
*/
