use crate::SharedMemoryBufferReader;
use circular_buffer::SizedReadGuard;
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

pub type SharedMemoryBufferReadGuard<'guard, T: ?Sized> =
    SizedReadGuard<'guard, SharedMemoryBufferReader, T>;

/// Macro to implement `CircularBufferReadable<SharedMemoryBufferReader>` for types
/// that implement `ReadableSharedMemoryBufferElement`.
///
/// This macro generates the implementation that would violate orphan rules if done
/// as a blanket implementation.
#[macro_export]
macro_rules! impl_circular_buffer_readable {
    ($type:ty) => {
        impl<'guard, 'buf>
            $crate::CircularBufferReadable<'guard, 'buf, $crate::SharedMemoryBufferReader> for $type
        where
            'buf: 'guard,
        {
            type ReadGuard
                = $crate::SharedMemoryBufferReadGuard<'guard, Self>
            where
                Self: 'guard;
            type ReadError = $crate::SharedMemoryTypedReadError;

            fn read(
                reader: &'guard mut $crate::SharedMemoryBufferReader,
                num: usize,
            ) -> Result<Self::ReadGuard, Self::ReadError> {
                use $crate::CircularBufferReader;

                let (primary_region, secondary_region) = reader.readable_region();

                let mut read_data = Vec::with_capacity(num);
                let mut advance_size = 0;
                let mut wrapped = false;

                for _i in 0..num {
                    let (current_region, offset) = if !wrapped {
                        if advance_size == primary_region.len()
                            || Self::check_wrap_flag(&primary_region[advance_size..])?
                        {
                            wrapped = true;
                            advance_size = primary_region.len();
                            (secondary_region, 0)
                        } else {
                            (primary_region, advance_size)
                        }
                    } else {
                        let offset = advance_size - primary_region.len();
                        (secondary_region, offset)
                    };

                    let element = Self::cast_to_element(&current_region[offset..])?;

                    // Untie lifetimes to allow MultiReadGuard to take both ref to reader and element
                    // The safety of this operation is based on the SharedMemoryBufferReadGuard's safety promises
                    let element_ptr = element as *const Self;
                    let element = unsafe { &*element_ptr };

                    let aligned_size =
                        ebutils::align_up_pow2(element.length_in_bytes(), reader.alignment_pow2());

                    if current_region.len() < aligned_size + offset {
                        return Err($crate::SharedMemoryTypedReadError::NotEnoughData);
                    }

                    read_data.push(element);
                    advance_size += aligned_size;
                }

                Ok($crate::SharedMemoryBufferReadGuard::from_reader(
                    reader,
                    read_data,
                    advance_size,
                ))
            }
        }
    };
}

// Blanket implementation for all types that implement `SharedMemoryBufferElement`.
// Violates orphan rules so a macro for the user to call is offered instead.
/*
impl<'guard, 'buf, T: ReadableSharedMemoryBufferElement> CircularBufferReadable<'guard, 'buf, SharedMemoryBufferReader> for T
where
    'buf: 'guard,
{
    type ReadGuard
    = SharedMemoryBufferReadGuard<'guard, Self>
    where
        Self: 'guard;
    type ReadError = SharedMemoryTypedReadError;

    fn read(
        reader: &'guard mut SharedMemoryBufferReader,
        num: usize,
    ) -> Result<Self::ReadGuard, Self::ReadError> {
        let (primary_region, secondary_region) = reader.readable_region();

        let mut read_data = Vec::with_capacity(num);
        let mut advance_size = 0;
        let mut wrapped = false;

        for _i in 0..num {
            let (current_region, offset) = if !wrapped {
                if advance_size == primary_region.len()
                    || Self::check_wrap_flag(&primary_region[advance_size..])?
                {
                    wrapped = true;
                    advance_size = primary_region.len();
                    (secondary_region, 0)
                } else {
                    (primary_region, advance_size)
                }
            } else {
                let offset = advance_size - primary_region.len();
                (secondary_region, offset)
            };

            let element = Self::cast_to_element(&current_region[offset..])?;

            // Untie lifetimes to allow MultiReadGuard to take both ref to reader and element
            // The safety of this operation is based on the SharedMemoryBufferReadGuard's safety promises
            let element_ptr = element as *const Self;
            let element = unsafe { &*element_ptr };

            let aligned_size =
                ebutils::align_up_pow2(element.length_in_bytes(), reader.alignment_pow2());

            if current_region.len() < aligned_size + offset {
                return Err(SharedMemoryTypedReadError::NotEnoughData);
            }

            read_data.push(element);
            advance_size += aligned_size;
        }

        Ok(SharedMemoryBufferReadGuard {
            reader,
            read_data,
            advance_size,
        })
    }
}


/// Blanket implementation for all types that implement `SharedMemoryBufferElement`.
impl<T> CircularBufferMultiReadable<SharedMemoryBufferReader> for T
where
    T: ReadableSharedMemoryBufferElement,
    for<'a> T: 'a,
{
    type MultiReadResult<'a> =
        Result<MultiReadGuard<'a, SharedMemoryBufferReader, Self>, SharedMemoryTypedReadError>;

    #[instrument(skip_all, fields(type = type_name::<T>(), shmem = reader.buffer_name()))]
    fn read_multiple(
        reader: &mut SharedMemoryBufferReader,
        num: usize,
    ) -> Self::MultiReadResult<'_> {
        debug!("Attempting to read multiple from shared memory buffer");

        debug!("Getting the buffer's readable region");
        let (primary_region, secondary_region) = reader.readable_region();

        let mut read_data = Vec::with_capacity(num);
        let mut advance_size = 0;
        let mut wrapped = false;

        for i in 0..num {
            debug!("Reading element {i} from the buffer");

            debug!(
                "Determining which region of the buffer's readable region \
                 to read from and what offset to start reading from"
            );
            let (current_region, offset) = if !wrapped {
                debug!("Not yet wrapped. Reading from primary region");
                debug!("Checking if wrap flag is present in primary region");
                if advance_size == primary_region.len()
                    || Self::check_wrap_flag(&primary_region[advance_size..]).map_err(|error| {
                        warn!("Unable to read the wrap flag from the primary region");
                        error
                    })?
                {
                    debug!(
                        "Wrap flag detected. Switching to secondary \
                        region until the end of the multiread"
                    );
                    wrapped = true;

                    debug!("Setting advance size to the length of the primary region");
                    advance_size = primary_region.len();

                    debug!(
                        "Reading from the secondary region \
                        starting initializing offset for it (0)"
                    );
                    (secondary_region, 0)
                } else {
                    debug!(
                        "No wrap flag detected. Reading from primary region with \
                        the current accumulated offset for it ({advance_size})"
                    );
                    (primary_region, advance_size)
                }
            } else {
                let offset = advance_size - primary_region.len();
                debug!(
                    "Already wrapped in a previous iteration. Reading from \
                    secondary region with accumulated offset for it ({offset})"
                );
                (secondary_region, offset)
            };

            debug!("Casting raw data to element for reading");
            let element = Self::cast_to_element(&current_region[offset..]).map_err(|error| {
                warn!("Unable to cast raw data to element for reading");
                error
            })?;

            debug!("Untying lifetimes to allow ReadGuard to take both ref to reader and element");
            debug!("The safety of this operation is based on the ReadGuard's safety promises");
            let element_ptr = element as *const Self;
            let element = unsafe { &*element_ptr };

            debug!("Verifying there is enough data to read this element and its alignment padding");
            let aligned_size =
                ebutils::align_up_pow2(element.length_in_bytes(), reader.alignment_pow2());
            if current_region.len() < aligned_size + offset {
                warn!("There is not enough data to read this element and its alignment padding");
                return Err(SharedMemoryTypedReadError::NotEnoughData);
            }

            debug!(
                "Element read successfully from the buffer storing \
                 its reference on a read multiple vector"
            );
            read_data.push(element);
            advance_size += aligned_size;
        }

        debug!("Elements read successfully from the buffer");
        Ok(MultiReadGuard::new(reader, read_data, advance_size))
    }
}
*/
