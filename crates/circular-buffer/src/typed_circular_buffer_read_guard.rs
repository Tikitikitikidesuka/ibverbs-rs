use crate::circular_buffer::CircularBufferReader;

pub struct ReadGuard<'a, R: CircularBufferReader, T> {
    reader: &'a mut R,
    data: &'a T,
    advance_size: usize,
}

impl<'a, R: CircularBufferReader, T> ReadGuard<'a, R, T> {
    pub fn new(reader: &'a mut R, data: &'a T, advance_size: usize) -> Self {
        Self {
            reader,
            data,
            advance_size,
        }
    }

    pub fn discard(self) -> R::AdvanceResult {
        self.reader.advance_read_pointer(self.advance_size)
    }
}

impl<'a, R: CircularBufferReader, T> std::ops::Deref for ReadGuard<'a, R, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

pub struct MultiReadGuard<'a, R: CircularBufferReader, T> {
    reader: &'a mut R,
    data: Vec<&'a T>,
    advance_size: usize,
}

impl<'a, R: CircularBufferReader, T> MultiReadGuard<'a, R, T> {
    pub fn new(reader: &'a mut R, data: Vec<&'a T>, advance_size: usize) -> Self {
        Self {
            reader,
            data,
            advance_size,
        }
    }

    pub fn discard(self) -> R::AdvanceResult {
        self.reader.advance_read_pointer(self.advance_size)
    }
}

impl<'a, R: CircularBufferReader, T> std::ops::Deref for MultiReadGuard<'a, R, T> {
    type Target = [&'a T];

    fn deref(&self) -> &Self::Target {
        self.data.as_slice()
    }
}
