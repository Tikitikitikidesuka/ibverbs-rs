use crate::circular_buffer::{CircularBufferReader, CircularBufferWriter};
use std::error::Error;
use std::ops::Deref;

// Since no extra header data can be added to either PCIe40 or SharedMemoryBuffer data
// without updating current protocols already in production, each type must define how its
// written and read from each buffer. This is the abstraction that allows for that.

pub trait CircularBufferReadable<'guard, 'buf, Reader: CircularBufferReader + 'buf>
where
    'buf: 'guard,
{
    type ReadGuard: ReadGuard<'guard, Reader, Self>
    where
        Self: 'guard,
        Reader: 'buf;
    type ReadError: Error;

    fn read(reader: &'guard mut Reader, num: usize) -> Result<Self::ReadGuard, Self::ReadError>;
}

pub trait ReadGuard<'guard, Reader: CircularBufferReader + ?Sized, T: ?Sized + 'guard>:
Deref<Target = [&'guard T]>
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
/*
pub trait CircularBufferTypedReader: CircularBufferReader {
    type ReadGuard<'guard, T: ?Sized + 'guard>: ReadGuard<'guard, Self, T>
    where
        Self: 'guard;
    type ReadError: Error;

    fn typed_read<T: ?Sized>(
        &mut self,
        num: usize,
    ) -> Result<Self::ReadGuard<'_, T>, Self::ReadError>;
}

pub trait ReadGuard<'guard, Reader: CircularBufferReader + ?Sized, T: ?Sized + 'guard>:
    Deref<Target = [&'guard T]>
{
    fn discard(self) -> Reader::AdvanceResult
    where
        Self: Sized;
}

pub trait CircularBufferTypedWriter: CircularBufferReader {
    type WriteError: Error;

    fn typed_write<T: ?Sized>(&mut self, data: &T) -> Result<(), Self::WriteError>;
}
*/
