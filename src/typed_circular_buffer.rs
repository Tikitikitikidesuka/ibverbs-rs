use crate::circular_buffer::{CircularBufferReader, CircularBufferWriter};

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

    fn read_multiple(reader: &mut R, num: usize) -> Self::MultiReadResult<'_>;
}

pub trait CircularBufferWritable<W: CircularBufferWriter> {
    type WriteResult;

    fn write(&self, writer: &mut W) -> Self::WriteResult;
}

// To write multiple structures, the user can call write many times or implement
// the writing abstraction over a collection type like Vec<T> or &[T]
