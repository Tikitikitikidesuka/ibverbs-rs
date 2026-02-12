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

// This is unsafe as the data references the reader. It is an invariant that the reader is not modified while the data may still be accessd.
// Todo: make this safer, this requires a neccesary refactor of the whole circular buffer trait system.
pub struct MultiReadGuard<'a, R: CircularBufferReader, T> {
    reader: &'a mut R,
    data: Vec<&'a T>,
    advance_sizes: Vec<usize>,
}

impl<'a, R: CircularBufferReader, T> MultiReadGuard<'a, R, T> {
    pub fn new(reader: &'a mut R, data: Vec<&'a T>, advance_sizes: Vec<usize>) -> Self {
        assert_eq!(data.len(), advance_sizes.len());
        Self {
            reader,
            data,
            advance_sizes,
        }
    }

    /// Truncates the guard to the given number of elements.
    ///
    /// All the truncated elements are **not** discarded but just kept in buffer for the next read.
    /// This is useful to just call [`Self::discard_all`] later.
    pub fn truncate(&mut self, new_num: usize) {
        assert!(new_num <= self.num_elements());
        self.data.truncate(new_num);
        self.advance_sizes.truncate(new_num);
    }

    pub fn discard_n(self, num: usize) -> R::AdvanceResult {
        self.reader.advance_read_pointer(self.advance_sizes[num])
    }

    pub fn discard_all(self) -> R::AdvanceResult {
        self.reader
            .advance_read_pointer(self.advance_sizes.last().copied().unwrap_or_default())
    }

    pub fn num_elements(&self) -> usize {
        self.data.len()
    }

    pub fn get_reader(&self) -> &R {
        self.reader
    }
}

impl<'a, R: CircularBufferReader, T> std::ops::Deref for MultiReadGuard<'a, R, T> {
    type Target = [&'a T];

    fn deref(&self) -> &Self::Target {
        self.data.as_slice()
    }
}
