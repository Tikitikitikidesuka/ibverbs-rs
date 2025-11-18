use crate::{CircularBufferReader, ReadGuard};
use std::ops::Deref;

pub struct SizedReadGuard<'guard, Reader: CircularBufferReader, T: ?Sized> {
    reader: &'guard mut Reader,
    read_data: Vec<&'guard T>,
    advance_size: usize,
}

impl<'guard, Reader: CircularBufferReader, T: ?Sized> SizedReadGuard<'guard, Reader, T> {
    pub fn from_reader(
        reader: &'guard mut Reader,
        read_data: impl IntoIterator<Item = &'guard T>,
        advance_size: usize,
    ) -> Self {
        Self {
            reader,
            read_data: read_data.into_iter().collect(),
            advance_size,
        }
    }
}

impl<'guard, Reader: CircularBufferReader, T: ?Sized> Deref for SizedReadGuard<'guard, Reader, T> {
    type Target = [&'guard T];

    fn deref(&self) -> &Self::Target {
        self.read_data.as_slice()
    }
}

impl<'guard, Reader: CircularBufferReader, T: ?Sized> ReadGuard<'guard, Reader, T>
    for SizedReadGuard<'guard, Reader, T>
{
    fn discard(self) -> Reader::AdvanceResult
    where
        Self: Sized,
    {
        self.reader.advance_read_pointer(self.advance_size)
    }
}
