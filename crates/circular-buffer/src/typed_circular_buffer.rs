use crate::circular_buffer::{CircularBufferReader, CircularBufferWriter};

// todo these traits are completely useless as almost everything about them is generic. There is not even a bound on what is returned by read etc.
// these need heavy refactoring and simplification to avoid needing to reimplement everything for every type that can be read by a "CircularBufferReader".

pub trait CircularBufferReadable<R: CircularBufferReader> {
    type ReadResult<'a>
    where
        R: 'a;

    fn read(reader: &mut R) -> Self::ReadResult<'_>;
}

pub trait CircularBufferMultiReadable<R: CircularBufferReader> {
    type MultiReadResult<'a>
    where
        R: 'a;

    /// Reads up to `num` elements from the reader.
    ///
    /// If less than `num` elements are available, the reader will read as many as possible.
    /// It is possible to discard less than the requested number of elements afterwards.
    fn read_multiple(reader: &mut R, num: usize) -> Self::MultiReadResult<'_>;
}

pub trait CircularBufferWritable<W: CircularBufferWriter> {
    type WriteResult;

    fn write(&self, writer: &mut W) -> Self::WriteResult;
}

// To write multiple structures, the user can call write many times or implement
// the writing abstraction over a collection type like Vec<T> or &[T]
