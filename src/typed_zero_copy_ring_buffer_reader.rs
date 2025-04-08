use crate::typed_zero_copy_ring_buffer_reader::ZeroCopyRingBufferReadableError::NotEnoughDataAvailable;
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

    #[error(
        "Improperly formatted data: {message}"
    )]
    ImproperlyFormattedData {
        message: String,
    }
}

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
        reader: &'buf mut R,
        offset: usize,
    ) -> Result<(DataGuard<'buf, R>, usize), ZeroCopyRingBufferReadableError>;

    /// Returns a reference to a T typed struct interpreting the data
    fn cast(data: &'buf [u8]) -> Result<Self, ZeroCopyRingBufferReadableError>;

    fn read(
        reader: &'buf mut R,
    ) -> Result<TypedDataGuard<'buf, R, Self>, ZeroCopyRingBufferReadableError> {
        let (data_guard, data_length) = Self::load(reader, 0)?;
        Ok(TypedDataGuard::new(data_guard)?)
    }

    /*
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
    */

    /*
    fn discard(
        reader: &'buf mut R,
        data: TypedDataGuard<'buf, R, Self>,
    ) -> Result<(), ZeroCopyRingBufferReadableError> {
        let data_length = data.byte_len();
        drop(data); // Release the guard on the reader

        let discarded_data_length = reader.discard_data(data_length).map_err(|error| {
            ZeroCopyRingBufferReadableError::ZeroCopyRingBufferReaderError(error)
        })?;

        if discarded_data_length != data_length {
            Err(NotEnoughDataAvailable {
                required_data: data_length,
                available_data: discarded_data_length,
            })?;
        }

        Ok(())
    }

    fn discard_multiple(
        reader: &'buf mut R,
        data: TypedMultiDataGuard<'buf, R, Self>,
    ) -> Result<(), ZeroCopyRingBufferReadableError> {
        let data_length = data.iter().fold(0, |acc, item| acc + item.byte_len());
        drop(data); // Release the guard on the reader

        let discarded_data_length = reader.discard_data(data_length).map_err(|error| {
            ZeroCopyRingBufferReadableError::ZeroCopyRingBufferReaderError(error)
        })?;

        if discarded_data_length != data_length {
            Err(NotEnoughDataAvailable {
                required_data: data_length,
                available_data: discarded_data_length,
            })?;
        }

        Ok(())
    }
    */
}

pub struct TypedDataGuard<'a, R: ZeroCopyRingBufferReader + ?Sized, T: ZeroCopyRingBufferReadable<R>> {
    data_guard: DataGuard<'a, R>,
    _phantom_type: PhantomData<T>,
}

/*
pub struct TypedMultiDataGuard<'a, R: ZeroCopyRingBufferReader + ?Sized, T> {
    data_guard: DataGuard<'a, R>,
    typed_data: Vec<T>,
}
*/

impl<'a, R: ZeroCopyRingBufferReader + ?Sized, T: ZeroCopyRingBufferReadable<R>> TypedDataGuard<'a, R, T> {
    pub fn new(data_guard: DataGuard<'a, R>) -> Result<Self, ZeroCopyRingBufferReadableError> {
        T::cast(data_guard.data_ref())?; // Ensure data is properly formated

        Ok(Self {
            data_guard,
            _phantom_type: PhantomData,
        })
    }

    pub fn data_ref(&self) -> &T {
        // Data is ensured to be properly formated on the `new` constructor but
        // A reference to the cast version cannot be held since multiple references would
        // be held to the data, one by the `DataGuard` and another from the cast data.
        // This cannot be since the `DataGuard` holds a mutable reference.
        // No need to check again, however. It should be impossible for it to not be correctly
        // formated here so an `unreachable` block will be used.

        match &T::cast(self.data_guard.data_ref()) {
            Ok(typed_data) => {typed_data}
            Err(_) => {
                unreachable!("\
                    Data should be guaranteed to be properly formatted when a `TypedDataGuard` is created.\n\
                    It should be impossible to reach an error on impropper format here since it was checked previously \
                    and the data is guaranteed not to change or be invalidated thanks to the `DataGuard`.\n\
                    This is therefore an implementation error, please contact the developers.
                ")
            }
        }
    }

    pub fn reader_ref(&self) -> &R {
        self.data_guard.reader_ref()
    }
}

/*
impl<'a, R: ZeroCopyRingBufferReader + ?Sized, T> TypedMultiDataGuard<'a, R, T> {
    pub fn new(data_guard: DataGuard<'a, R>, typed_data: Vec<T>) -> Self {
        Self {
            data_guard,
            typed_data,
        }
    }
}
*/

impl<'a, R: ZeroCopyRingBufferReader + ?Sized, T: ZeroCopyRingBufferReadable<R>> Deref for TypedDataGuard<'a, R, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data_ref()
    }
}

/*
impl<'a, R: ZeroCopyRingBufferReader + ?Sized, T> Deref for TypedMultiDataGuard<'a, R, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.typed_data.as_slice()
    }
}
*/