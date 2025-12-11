use std::ops::Range;

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
    /// ranges in the underlying buffer where the data lies
    ranges: Vec<Range<usize>>,
    advance_size: usize,
}

impl<'a, R: CircularBufferReader, T> MultiReadGuard<'a, R, T> {
    pub fn new(
        reader: &'a mut R,
        data: Vec<&'a T>,
        ranges: Vec<Range<usize>>,
        advance_size: usize,
    ) -> Self {
        Self {
            reader,
            data,
            ranges,
            advance_size,
        }
    }

    pub fn discard(self) -> R::AdvanceResult {
        self.reader.advance_read_pointer(self.advance_size)
    }

    pub fn get_reader(&self) -> &R {
        self.reader
    }

    /// iterator over ranges of indices in the underlying buffer where the data lies.
    ///
    /// Useful if used with DMA.
    pub fn ranges(&self) -> impl Iterator<Item = Range<usize>> {
        self.ranges.iter().cloned()
    }
}

impl<'a, R: CircularBufferReader, T> std::ops::Deref for MultiReadGuard<'a, R, T> {
    type Target = [&'a T];

    fn deref(&self) -> &Self::Target {
        self.data.as_slice()
    }
}
