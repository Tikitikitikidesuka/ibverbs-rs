use std::error::Error;
use crate::circular_buffer::{CircularBufferReader, CircularBufferWriter};
use std::ops::Deref;

pub trait CircularBufferReadable<'guard, 'buf, Reader: CircularBufferReader + 'buf>
where 'buf: 'guard {
    type ReadGuard: ReadGuard<'guard, Reader, Self>
    where
        Self: 'guard, Reader: 'buf;
    type ReadError: Error;

    fn read(reader: &'guard mut Reader, num: usize) -> Result<Self::ReadGuard, Self::ReadError>;
}

pub trait ReadGuard<'a, Reader: CircularBufferReader, T: ?Sized + 'a>:
Deref<Target = [&'a T]>
{
    fn discard(self) -> Reader::AdvanceResult
    where
        Self: Sized;
}

pub trait CircularBufferWritable<Writer: CircularBufferWriter> {
    type WriteError: Error;

    fn write(&self, writer: &mut Writer) -> Result<(), Self::WriteError>;
}

// To write multiple structures, the user can call write many times or implement
// the writing abstraction over a collection type like Vec<T> or &[T]
