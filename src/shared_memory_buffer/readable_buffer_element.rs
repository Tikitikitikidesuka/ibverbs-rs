use crate::circular_buffer::CircularBufferReader;
use crate::shared_memory_buffer::buffer_element::SharedMemoryBufferElement;
use crate::shared_memory_buffer::reader::SharedMemoryBufferReader;
use crate::typed_circular_buffer::{CircularBufferMultiReadable, CircularBufferReadable};
use crate::typed_circular_buffer_read_guard::{MultiReadGuard, ReadGuard};
use crate::utils;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SharedMemoryTypedReadError {
    #[error("Type not found on buffer")]
    NotFound,

    #[error("Not enough data for requested type")]
    NotEnoughData,

    #[error("Data is corrupt for requested type")]
    CorruptData,
}

/// Blanket implementation for all types that implement `SharedMemoryBufferElement`.
impl<T> CircularBufferReadable<SharedMemoryBufferReader> for T
where
    T: SharedMemoryBufferElement,
    for<'a> T: 'a,
{
    type ReadResult<'a> =
        Result<ReadGuard<'a, SharedMemoryBufferReader, Self>, SharedMemoryTypedReadError>;

    fn read(reader: &mut SharedMemoryBufferReader) -> Self::ReadResult<'_> {
        let (primary_region, secondary_region) = reader.readable_region();

        // Determine which region to read from based on wrap flag
        let (readable_region, region_offset) = if Self::check_wrap_flag(primary_region)? {
            (secondary_region, primary_region.len())
        } else {
            (primary_region, 0)
        };

        // Cast to element
        let element = Self::cast_to_element(readable_region)?;

        // Untie lifetimes so ReadGuard can take both ref to reader and element
        let element_ptr = element as *const Self;
        let element = unsafe { &*element_ptr };

        // Verify there is enough data with alignment
        let aligned_size = utils::align_up_pow2(element.length_in_bytes(), reader.alignment_pow2());
        if readable_region.len() < aligned_size {
            return Err(SharedMemoryTypedReadError::NotEnoughData);
        }

        // Calculate total discard size
        let discard_size = region_offset + aligned_size;

        Ok(ReadGuard::new(reader, element, discard_size))
    }
}

/// Blanket implementation for all types that implement `SharedMemoryBufferElement`.
impl<T> CircularBufferMultiReadable<SharedMemoryBufferReader> for T
where
    T: SharedMemoryBufferElement,
    for<'a> T: 'a,
{
    type MultiReadResult<'a> =
        Result<MultiReadGuard<'a, SharedMemoryBufferReader, Self>, SharedMemoryTypedReadError>;

    fn read_multiple(
        reader: &mut SharedMemoryBufferReader,
        num: usize,
    ) -> Self::MultiReadResult<'_> {
        let (primary_region, secondary_region) = reader.readable_region();
        let mut read_data = Vec::with_capacity(num);
        let mut advance_size = 0;
        let mut wrapped = false;

        for _ in 0..num {
            // Determine current reading position and region
            let (current_region, offset) = if !wrapped {
                if Self::check_wrap_flag(&primary_region[advance_size..])? {
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

            // Cast to element
            let element = Self::cast_to_element(current_region)?;
            
            // Untie lifetimes so ReadGuard can take both ref to reader and element
            let element_ptr = element as *const Self;
            let element = unsafe { &*element_ptr };

            // Calculate entry size and validate total space
            let aligned_size =
                utils::align_up_pow2(element.length_in_bytes(), reader.alignment_pow2());

            if current_region.len() < aligned_size + offset {
                return Err(SharedMemoryTypedReadError::NotEnoughData);
            }

            // Store entry and advance position
            read_data.push(element);
            advance_size += aligned_size;
        }

        Ok(MultiReadGuard::new(reader, read_data, advance_size))
    }
}
