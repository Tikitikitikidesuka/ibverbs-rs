use crate::circular_buffer::CircularBufferReader;

/// RAII guard for zero-copy single reads from circular buffers.
///
/// `ReadGuard` provides safe, zero-copy access to data read from a circular buffer
/// while holding a mutable borrow of the reader. This ensures exclusive read access
/// and prevents the reader from being used elsewhere while data is being processed.
///
/// The guard does NOT automatically advance the read pointer when dropped. The read
/// pointer must be advanced explicitly by calling [`discard()`](Self::discard), which
/// consumes the guard and returns the result of advancing by `advance_size` bytes.
/// This design allows inspection of data without committing to consumption, enabling
/// validation or conditional processing.
///
/// # Type Parameters
///
/// * `'a` - Lifetime of the borrow on both the reader and the data
/// * `R` - The circular buffer reader type implementing [`CircularBufferReader`]
/// * `T` - The type being read from the buffer
pub struct ReadGuard<'a, R: CircularBufferReader, T> {
    reader: &'a mut R,
    data: &'a T,
    advance_size: usize,
}

impl<'a, R: CircularBufferReader, T> ReadGuard<'a, R, T> {
    /// Creates a new read guard.
    ///
    /// # Arguments
    ///
    /// * `reader` - Mutable reference to the circular buffer reader
    /// * `data` - Reference to the data read from the buffer (must have lifetime tied to reader)
    /// * `advance_size` - Number of bytes to advance when the guard is discarded
    ///
    /// # Safety
    ///
    /// The caller must ensure that `data` points to valid memory within the reader's
    /// readable region and that `advance_size` correctly represents the aligned size
    /// of the data being guarded.
    pub fn new(reader: &'a mut R, data: &'a T, advance_size: usize) -> Self {
        Self {
            reader,
            data,
            advance_size,
        }
    }

    /// Consumes the guard and advances the read pointer.
    ///
    /// This explicitly commits the read operation by advancing the reader's pointer
    /// by length of the read data. The result depends on the reader's `advance_read_pointer` implementation.
    ///
    /// # Returns
    ///
    /// The result of advancing the read pointer, as defined by the reader's
    /// [`CircularBufferReader::AdvanceResult`] associated type.
    pub fn discard(self) -> R::AdvanceResult {
        self.reader.advance_read_pointer(self.advance_size)
    }
}

impl<'a, R: CircularBufferReader, T> std::ops::Deref for ReadGuard<'a, R, T> {
    type Target = T;

    /// Provides transparent access to the guarded data.
    ///
    /// This allows using the guard as if it were a direct reference to `T`,
    /// enabling zero-copy access patterns like `guard.some_method()` instead
    /// of requiring `(*guard).some_method()`.
    fn deref(&self) -> &Self::Target {
        self.data
    }
}

/// RAII guard for zero-copy batch reads from circular buffers.
///
/// `MultiReadGuard` extends the [`ReadGuard`] pattern to multiple entries read in
/// a single operation. It provides slice-like access to multiple data references
/// while holding a mutable borrow of the reader.
///
/// Like `ReadGuard`, this does NOT automatically advance the read pointer when dropped.
/// The read pointer must be advanced explicitly by calling [`discard()`](Self::discard),
/// which advances by the cumulative `advance_size` of all entries.
///
/// # Type Parameters
///
/// * `'a` - Lifetime of the borrow on both the reader and all data references
/// * `R` - The circular buffer reader type implementing [`CircularBufferReader`]
/// * `T` - The type of elements being read from the buffer
pub struct MultiReadGuard<'a, R: CircularBufferReader, T> {
    reader: &'a mut R,
    data: Vec<&'a T>,
    advance_size: usize,
}

impl<'a, R: CircularBufferReader, T> MultiReadGuard<'a, R, T> {
    /// Creates a new multi-read guard.
    ///
    /// # Arguments
    ///
    /// * `reader` - Mutable reference to the circular buffer reader
    /// * `data` - Vector of references to entries read from the buffer
    /// * `advance_size` - Cumulative bytes to advance when the guard is discarded
    ///
    /// # Safety
    ///
    /// The caller must ensure that all references in `data` point to valid memory
    /// within the reader's readable region and that `advance_size` correctly
    /// represents the total aligned size of all entries being guarded.
    pub fn new(reader: &'a mut R, data: Vec<&'a T>, advance_size: usize) -> Self {
        Self {
            reader,
            data,
            advance_size,
        }
    }

    /// Consumes the guard and advances the read pointer.
    ///
    /// This explicitly commits the batch read operation by advancing the reader's
    /// pointer by the cumulative `advance_size` bytes covering all entries.
    /// The result depends on the reader's `advance_read_pointer` implementation.
    ///
    /// # Returns
    ///
    /// The result of advancing the read pointer, as defined by the reader's
    /// [`CircularBufferReader::AdvanceResult`] associated type.
    pub fn discard(self) -> R::AdvanceResult {
        self.reader.advance_read_pointer(self.advance_size)
    }
}

impl<'a, R: CircularBufferReader, T> std::ops::Deref for MultiReadGuard<'a, R, T> {
    type Target = [&'a T];

    /// Provides transparent slice access to the guarded data.
    ///
    /// This allows using the guard as if it were a slice of references,
    /// enabling patterns like `guard.len()`, `guard.iter()`, or `guard[0]`
    /// without manual dereferencing.
    fn deref(&self) -> &Self::Target {
        self.data.as_slice()
    }
}
