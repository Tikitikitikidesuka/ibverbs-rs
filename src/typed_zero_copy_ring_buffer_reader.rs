use crate::zero_copy_ring_buffer_reader::{
    DataGuard, ZeroCopyRingBufferReader, ZeroCopyRingBufferReaderError,
};
use std::marker::PhantomData;
use std::ops::Deref;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ZeroCopyRingBufferReadableError {
    #[error("{0}")]
    ZeroCopyRingBufferReaderError(ZeroCopyRingBufferReaderError),

    #[error(
        "Not enough data available: Required {required_data} bytes. Only {available_data} bytes are available in the buffer"
    )]
    NotEnoughDataAvailable {
        required_data: usize,
        available_data: usize,
    },
}

/* The read method failed because I was returning a struct with a dataguard and an attribute referencing it.
Rust does not allow self referencing structs. A solution to it would be decomposing the DataGuard and having its components raw
This might be the next best solution:

TypedDataGuard:
- reader reference
- typed data struct

Then I can keep the load and cast methods for the user to implement and a defualt read one that uses them both and combines their output:
fn read(reader) {
    let data_guard = load(reader);
    ... actually I think this will also count as self referencing
    ... or as in dropping the data_guard. I should try and see
    ... Gonna have lunch now hehe
}
 */

/// Helper function to guarantee the buffer has at least `required_bytes` available
/// Useful for many `ZeroCopyRingBufferReadable` implementations
pub fn ensure_available_bytes<R: ZeroCopyRingBufferReader + ?Sized>(
    reader: &mut R,
    required_bytes: usize,
) -> Result<(), ZeroCopyRingBufferReadableError> {
    let available_data = reader.data().len();

    if available_data < required_bytes {
        // Try to load more data
        let loaded_data = reader
            .load_data(required_bytes - available_data)
            .map_err(|error| {
                ZeroCopyRingBufferReadableError::ZeroCopyRingBufferReaderError(error)
            })?;

        // Check if we have enough data now
        if available_data + loaded_data < required_bytes {
            Err(ZeroCopyRingBufferReadableError::NotEnoughDataAvailable {
                required_data: required_bytes,
                available_data: available_data + loaded_data,
            })?;
        }
    }

    Ok(())
}

pub trait ZeroCopyRingBufferReadable<'buf, R: ZeroCopyRingBufferReader + ?Sized>: Sized {
    /// Finds a T typed struct's data in the reader's buffer.
    /// It assumes the first byte of the struct is at offset on the buffer from the read_pointer.
    /// Loads more data if necessary. Returns the size of the loaded struct.
    fn load(
        reader: &mut R,
        offset: usize,
    ) -> Result<(DataGuard<R>, usize), ZeroCopyRingBufferReadableError>;

    /// Returns a reference to a T typed struct interpreting the data
    fn cast(data: &'buf [u8]) -> Result<Self, ZeroCopyRingBufferReadableError>;

    fn read(
        reader: &'buf mut R,
    ) -> Result<TypedDataGuard<'buf, R, Self>, ZeroCopyRingBufferReadableError> {
        let (data_guard, data_length) = Self::load(reader, 0)?;
        let typed_data = Self::cast(&data_guard.data_ref()[..data_length])?;
        Ok(TypedDataGuard::new(data_guard, typed_data))
    }

    fn read_multiple(
        reader: &'buf mut R,
        count: usize,
    ) -> Result<TypedMultiDataGuard<'buf, R, Self>, ZeroCopyRingBufferReadableError> {
        // First pass: determine total size and sizes of each element
        let mut sizes = Vec::with_capacity(count);
        let mut total_size = 0;

        for _ in 0..count {
            let data_length = {
                let (_, length) = Self::load(reader, total_size).map_err(|error| {
                    if let ZeroCopyRingBufferReadableError::NotEnoughDataAvailable {
                        required_data,
                        available_data,
                    } = error
                    {
                        ZeroCopyRingBufferReadableError::NotEnoughDataAvailable {
                            required_data: total_size + required_data,
                            available_data: available_data + available_data,
                        }
                    } else {
                        error
                    }
                })?;
                length
            };

            sizes.push(data_length);
            total_size += data_length;
        }

        // Now get a single DataGuard that covers all the data
        let data_guard = reader.data();

        // Cast all elements with accumulating offset
        let mut elements = Vec::with_capacity(count);
        let mut current_offset = 0;

        for size in sizes {
            let element =
                Self::cast(&data_guard.data_ref()[current_offset..current_offset + size])?;
            elements.push(element);
            current_offset += size;
        }

        // Create and return the TypedMultiDataGuard
        Ok(TypedMultiDataGuard::new(data_guard, elements))
    }
}

pub struct TypedDataGuard<'a, R: ZeroCopyRingBufferReader + ?Sized, T> {
    data_guard: DataGuard<'a, R>,
    typed_data: T,
}

pub struct TypedMultiDataGuard<'a, R: ZeroCopyRingBufferReader + ?Sized, T> {
    data_guard: DataGuard<'a, R>,
    typed_data: Vec<T>,
}

impl<'a, R: ZeroCopyRingBufferReader + ?Sized, T> TypedDataGuard<'a, R, T> {
    pub fn new(data_guard: DataGuard<'a, R>, typed_data: T) -> Self {
        Self {
            data_guard,
            typed_data,
        }
    }
}

impl<'a, R: ZeroCopyRingBufferReader + ?Sized, T> TypedMultiDataGuard<'a, R, T> {
    pub fn new(data_guard: DataGuard<'a, R>, typed_data: Vec<T>) -> Self {
        Self {
            data_guard,
            typed_data,
        }
    }
}

impl<'a, R: ZeroCopyRingBufferReader + ?Sized, T> Deref for TypedDataGuard<'a, R, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.typed_data
    }
}

impl<'a, R: ZeroCopyRingBufferReader + ?Sized, T> Deref for TypedMultiDataGuard<'a, R, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.typed_data.as_slice()
    }
}
