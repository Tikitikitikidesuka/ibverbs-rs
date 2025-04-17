use crate::zero_copy_ring_buffer_reader::{
    DataGuard, ZeroCopyRingBufferReader, ZeroCopyRingBufferReaderError,
};
use std::fmt::{Debug, Display};
use std::marker::PhantomData;
use std::ops::{Deref, Index};
use std::time::Duration;
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

    #[error("Improperly formatted data: {message}")]
    ImproperlyFormattedData { message: String },
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
    /// It is used by read with offset zero to load one element. Also used by read_multiple with
    /// incrementing offsets to load multiple elements.
    fn load(reader: &mut R, offset: usize) -> Result<usize, ZeroCopyRingBufferReadableError>;

    /// Returns a reference to a T typed struct interpreting the data
    fn cast(data: &[u8]) -> Result<&Self, ZeroCopyRingBufferReadableError>;

    fn read(
        reader: &'buf mut R,
    ) -> Result<TypedDataGuard<'buf, R, Self>, ZeroCopyRingBufferReadableError> {
        let data_length = Self::load(reader, 0)?;
        TypedDataGuard::new(reader.data(), data_length)
    }

    // TODO: ADD READ_BLOCKING
    //fn read_blocking(
    //reader: &'buf mut R,
    //timeout: Duration,
    //) -> Result<TypedDataGuard<'buf, R, Self>, ZeroCopyRingBufferReadableError> {
    //
    //}

    fn read_multiple(
        reader: &'buf mut R,
        count: usize,
    ) -> Result<TypedMultiDataGuard<'buf, R, Self>, ZeroCopyRingBufferReadableError> {
        let mut offset = 0;
        let mut offsets = Vec::with_capacity(count);

        for _ in 0..count {
            offsets.push(offset);
            let data_length = Self::load(reader, offset)?;
            offset += data_length;
        }

        TypedMultiDataGuard::new(reader.data(), offsets, offset)
    }

    // TODO: ADD READ_MULTIPLE_BLOCKING
    //fn read_multiple_blocking(
    //reader: &'buf mut R,
    //count: usize,
    //timeout: Duration,
    //) -> Result<TypedMultiDataGuard<'buf, R, Self>, ZeroCopyRingBufferReadableError> {
    //
    //}
}

pub struct TypedDataGuard<
    'buf,
    R: ZeroCopyRingBufferReader + ?Sized,
    T: ZeroCopyRingBufferReadable<'buf, R>,
> {
    data_guard: DataGuard<'buf, R>,
    data_length: usize,
    _phantom_type: PhantomData<&'buf T>,
}

pub struct TypedMultiDataGuard<
    'buf,
    R: ZeroCopyRingBufferReader + ?Sized,
    T: ZeroCopyRingBufferReadable<'buf, R>,
> {
    data_guard: DataGuard<'buf, R>,
    data_length: usize,
    offsets: Vec<usize>,
    _phantom_type: PhantomData<&'buf T>,
}

impl<'buf, R: ZeroCopyRingBufferReader + ?Sized, T: ZeroCopyRingBufferReadable<'buf, R>>
    TypedDataGuard<'buf, R, T>
{
    pub fn new(
        data_guard: DataGuard<'buf, R>,
        data_length: usize,
    ) -> Result<Self, ZeroCopyRingBufferReadableError> {
        // Ensure data is properly formatted
        T::cast(data_guard.data_ref())?;

        Ok(Self {
            data_guard,
            data_length,
            _phantom_type: PhantomData,
        })
    }

    pub fn discard(self) -> Result<(), ZeroCopyRingBufferReadableError> {
        self.data_guard
            .discard_count(self.data_length)
            .map(|_| ())
            .map_err(ZeroCopyRingBufferReadableError::ZeroCopyRingBufferReaderError)
    }

    pub fn data_ref(&self) -> &T {
        // Data is ensured to be properly formated on the `new` constructor but
        // A reference to the cast version cannot be held since multiple references would
        // be held to the data, one by the `DataGuard` and another from the cast data.
        // This cannot be since the `DataGuard` holds a mutable reference.
        // No need to check again, however. It should be impossible for it to not be correctly
        // formated here so an `unreachable` block will be used.

        T::cast(self.data_guard.deref()).unwrap_or_else(move |_| {
            unreachable!("\
                    Data should be guaranteed to be properly formatted when a `TypedDataGuard` is created.\n\
                    It should be impossible to reach an error on impropper format here since it was checked previously \
                    and the data is guaranteed not to change or be invalidated thanks to the `DataGuard`.\n\
                    This is therefore an implementation error, please contact the developers.
                ")
        })
    }

    pub fn reader_ref(&self) -> &R {
        self.data_guard.reader_ref()
    }
}

impl<'buf, R: ZeroCopyRingBufferReader + ?Sized, T: ZeroCopyRingBufferReadable<'buf, R>>
    TypedMultiDataGuard<'buf, R, T>
{
    pub fn new(
        data_guard: DataGuard<'buf, R>,
        offsets: Vec<usize>,
        data_length: usize,
    ) -> Result<Self, ZeroCopyRingBufferReadableError> {
        // Ensure data is properly formatted
        for index in 0..offsets.len() {
            let start_idx = offsets[index];
            let end_idx = if index + 1 < offsets.len() {
                offsets[index + 1]
            } else {
                data_guard.len()
            };

            T::cast(&data_guard.data_ref()[start_idx..end_idx])?;
        }

        Ok(Self {
            data_guard,
            offsets,
            data_length,
            _phantom_type: PhantomData,
        })
    }

    pub fn discard(self) -> Result<(), ZeroCopyRingBufferReadableError> {
        self.data_guard
            .discard_count(self.data_length)
            .map(|_| ())
            .map_err(ZeroCopyRingBufferReadableError::ZeroCopyRingBufferReaderError)
    }

    pub fn data_ref(&self, index: usize) -> &T {
        let start_idx = self.offsets[index];
        let end_idx = if index + 1 < self.offsets.len() {
            self.offsets[index + 1]
        } else {
            self.data_guard.len()
        };

        let data = &self.data_guard.data_ref()[start_idx..end_idx];
        T::cast(data).unwrap_or_else(move |_| {
            unreachable!("\
                    Data should be guaranteed to be properly formatted when a `TypedDataGuard` is created.\n\
                    It should be impossible to reach an error on impropper format here since it was checked previously \
                    and the data is guaranteed not to change or be invalidated thanks to the `DataGuard`.\n\
                    This is therefore an implementation error, please contact the developers.
                ")
        })
    }

    pub fn reader_ref(&self) -> &R {
        self.data_guard.reader_ref()
    }

    pub fn iter<'a>(&'a self) -> TypedMultiDataGuardIter<'a, 'buf, R, T>
    where
        'buf: 'a, // 'buf outlives 'a
    {
        TypedMultiDataGuardIter::new(self)
    }
}

impl<'buf, R: ZeroCopyRingBufferReader + ?Sized, T: ZeroCopyRingBufferReadable<'buf, R>> Deref
    for TypedDataGuard<'buf, R, T>
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data_ref()
    }
}

impl<'buf, R: ZeroCopyRingBufferReader + ?Sized, T: ZeroCopyRingBufferReadable<'buf, R>>
    Index<usize> for TypedMultiDataGuard<'buf, R, T>
{
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.data_ref(index)
    }
}

pub struct TypedMultiDataGuardIter<
    'a,   // Lifetime of the reference to TypedMultiDataGuard
    'buf, // Original 'buf lifetime
    R: ZeroCopyRingBufferReader + ?Sized,
    T: ZeroCopyRingBufferReadable<'buf, R>,
> {
    typed_multi_data_guard: &'a TypedMultiDataGuard<'buf, R, T>,
    index: usize,
}

impl<'a, 'buf, R: ZeroCopyRingBufferReader + ?Sized, T: ZeroCopyRingBufferReadable<'buf, R>>
    TypedMultiDataGuardIter<'a, 'buf, R, T>
{
    pub fn new(typed_multi_data_guard: &'a TypedMultiDataGuard<'buf, R, T>) -> Self {
        Self {
            typed_multi_data_guard,
            index: 0,
        }
    }
}

impl<'a, 'buf, R: ZeroCopyRingBufferReader + ?Sized, T: ZeroCopyRingBufferReadable<'buf, R>>
    Iterator for TypedMultiDataGuardIter<'a, 'buf, R, T>
where
    'buf: 'a,
{
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.typed_multi_data_guard.offsets.len() {
            // Fixed this from data_guard.len()
            let result = self.typed_multi_data_guard.data_ref(self.index);
            self.index += 1;
            Some(result)
        } else {
            None
        }
    }
}

impl<'buf, R: ZeroCopyRingBufferReader + ?Sized, T: ZeroCopyRingBufferReadable<'buf, R>> Debug
    for TypedDataGuard<'buf, R, T>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TypedDataGuard")
            .field("type", &std::any::type_name::<T>())
            .field("data_guard", &self.data_guard)
            .finish()
    }
}

impl<'buf, R: ZeroCopyRingBufferReader + ?Sized, T: ZeroCopyRingBufferReadable<'buf, R> + Debug>
    Debug for TypedMultiDataGuard<'buf, R, T>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let formated_data = self.iter().collect::<Vec<_>>();

        f.debug_struct("TypedMultiDataGuard")
            .field("type", &std::any::type_name::<T>())
            .field("data_guard", &self.data_guard)
            .field("offsets", &self.offsets)
            .field("typed_data", &formated_data.as_slice())
            .finish()
    }
}

impl<'buf, R: ZeroCopyRingBufferReader + ?Sized, T: ZeroCopyRingBufferReadable<'buf, R> + Debug>
    Debug for TypedMultiDataGuardIter<'_, 'buf, R, T>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TypedMultiDataGuardIter")
            .field("type", &std::any::type_name::<T>())
            .field("index", &self.index)
            .field("typed_multi_data_guard", &self.typed_multi_data_guard)
            .field("total_items", &self.typed_multi_data_guard.offsets.len())
            .finish()
    }
}

impl<'buf, R: ZeroCopyRingBufferReader + ?Sized, T: ZeroCopyRingBufferReadable<'buf, R> + Display>
    Display for TypedDataGuard<'buf, R, T>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TypedDataGuard<{}>({})",
            std::any::type_name::<T>(),
            self.data_ref()
        )
    }
}

impl<'buf, R: ZeroCopyRingBufferReader + ?Sized, T: ZeroCopyRingBufferReadable<'buf, R> + Display>
    Display for TypedMultiDataGuard<'buf, R, T>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Create a formatted string for all elements
        let elements_str = self
            .iter()
            .map(|item| format!("{}", item))
            .collect::<Vec<_>>()
            .join(", ");

        write!(
            f,
            "TypedMultiDataGuard<{}>[{}]([{}])",
            std::any::type_name::<T>(),
            self.offsets.len(),
            elements_str
        )
    }
}

impl<'buf, R: ZeroCopyRingBufferReader + ?Sized, T: ZeroCopyRingBufferReadable<'buf, R> + Display>
    Display for TypedMultiDataGuardIter<'_, 'buf, R, T>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let current_item = if self.index < self.typed_multi_data_guard.offsets.len() {
            format!("{}", self.typed_multi_data_guard.data_ref(self.index))
        } else {
            "end".to_string()
        };

        write!(
            f,
            "TypedMultiDataGuardIter<{}>[{}/{}]({})",
            std::any::type_name::<T>(),
            self.index,
            self.typed_multi_data_guard.offsets.len(),
            current_item
        )
    }
}
