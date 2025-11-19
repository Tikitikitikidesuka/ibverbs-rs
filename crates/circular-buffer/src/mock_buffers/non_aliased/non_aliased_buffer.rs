use crate::{CircularBufferReader, CircularBufferWriter};
use std::convert::Infallible;
use std::ptr::{slice_from_raw_parts, slice_from_raw_parts_mut};
use std::sync::{Arc, Mutex};
use thiserror::Error;

/// A mock implementation of a non-aliased single-producer single-consumer ring buffer.
///
/// Unlike [`MockAliasedBuffer`](crate::mock_buffers::MockAliasedBuffer), this buffer does not
/// use memory aliasing. Instead, readable and writable regions may be fragmented into two
/// separate slices when data wraps around the buffer boundary. The caller is responsible
/// for handling this fragmentation.
#[derive(Debug, Clone)]
pub struct MockNonAliasedBuffer {
    inner: Arc<Mutex<MockNonAliasedBufferInner>>,
    alignment_pow2: u8,
}

#[derive(Debug)]
pub struct MockNonAliasedBufferInner {
    read_ptr: usize,
    write_ptr: usize,
    same_page: bool,
    read_locked: bool,
    write_locked: bool,
    buffer: Vec<u8>,
}

impl MockNonAliasedBuffer {
    /// Creates a new mock non-aliased buffer with the specified capacity and alignment.
    ///
    /// # Arguments
    ///
    /// * `capacity` - The capacity of the buffer in bytes. Must be aligned to
    ///   `2^alignment_pow2`.
    /// * `alignment_pow2` - Power of 2 for alignment requirements. For example, `3` means
    ///   8-byte alignment (2^3 = 8).
    ///
    /// # Returns
    ///
    /// * `Ok(Self)` - Successfully created buffer with both read and write pointers at position 0
    /// * `Err(&'static str)` - Capacity is not properly aligned to the required boundary
    ///
    /// # Examples
    ///
    /// ```
    /// # use circular_buffer::mock_buffers::MockNonAliasedBuffer;
    /// #
    /// // Create a 1024-byte buffer with 8-byte alignment
    /// let buffer = MockNonAliasedBuffer::new(1024, 3).unwrap();
    ///
    /// // This will fail because 1023 is not 8-byte aligned
    /// assert!(MockNonAliasedBuffer::new(1023, 3).is_err());
    /// ```
    pub fn new(capacity: usize, alignment_pow2: u8) -> Result<Self, &'static str> {
        if !ebutils::check_alignment_pow2(capacity, alignment_pow2) {
            Err("Capacity does not match alignment")
        } else {
            Ok(Self {
                inner: Arc::new(Mutex::new(MockNonAliasedBufferInner {
                    read_ptr: 0,
                    write_ptr: 0,
                    read_locked: false,
                    write_locked: false,
                    same_page: true,
                    buffer: vec![0; capacity],
                })),
                alignment_pow2,
            })
        }
    }

    /// Returns `true` if the buffer currently has an active reader.
    ///
    /// This flag is set when a [`MockNonAliasedBufferReader`] is created and cleared
    /// when it is dropped. It prevents multiple readers from being created simultaneously.
    pub fn read_locked(&self) -> bool {
        self.inner.lock().unwrap().read_locked
    }

    /// Returns `true` if the buffer currently has an active writer.
    ///
    /// This flag is set when a [`MockNonAliasedBufferWriter`] is created and cleared
    /// when it is dropped. It prevents multiple writers from being created simultaneously.
    pub fn write_locked(&self) -> bool {
        self.inner.lock().unwrap().write_locked
    }
}

/// A reader for [`MockNonAliasedBuffer`] that implements [`CircularBufferReader`].
///
/// Provides read-only access to the buffer's readable region and manages the read
/// pointer position. Unlike the aliased buffer, readable data may be fragmented into
/// two separate slices when it wraps around the buffer boundary.
///
/// Only one reader can exist for a buffer at a time. Attempting to create a second
/// reader will fail. The buffer is automatically unlocked when the reader is dropped.
///
/// # Safety
///
/// Contains a raw pointer to the underlying buffer. Ring buffers are generally external
/// to the program (DMA, RDMA, inter-process communications, etc.) so there is no way of
/// ensuring the buffer outlives the reader. It is the responsibility of the user to
/// ensure this.
pub struct MockNonAliasedBufferReader {
    buffer: MockNonAliasedBuffer,
}

impl MockNonAliasedBufferReader {
    /// Creates a new reader for the given buffer.
    ///
    /// Only one reader can exist for a buffer at a time. This constructor checks
    /// whether the buffer already has an active reader and fails if one exists.
    /// The reader lock is automatically set upon successful creation and released
    /// when the reader is dropped.
    ///
    /// # Arguments
    ///
    /// * `buffer` - Mutable reference to the buffer to read from
    ///
    /// # Returns
    ///
    /// * `Ok(Self)` - Successfully created reader with exclusive read access
    /// * `Err(&'static str)` - Buffer already has an active reader
    ///
    /// # Examples
    ///
    /// ```
    /// # use circular_buffer::mock_buffers::{MockNonAliasedBuffer, MockNonAliasedBufferReader};
    /// #
    /// let mut buffer = MockNonAliasedBuffer::new(1024, 3).unwrap();
    /// let reader = unsafe { MockNonAliasedBufferReader::new(buffer.clone()).unwrap() };
    ///
    /// // This will fail because a reader already exists
    /// assert!(unsafe { MockNonAliasedBufferReader::new(buffer.clone()).is_err() });
    /// ```
    pub fn new(buffer: MockNonAliasedBuffer) -> Result<Self, &'static str> {
        let mut buffer_guard = buffer.inner.lock().unwrap();
        if buffer_guard.read_locked {
            return Err("Buffer already has a reader");
        }
        buffer_guard.read_locked = true;
        drop(buffer_guard);
        Ok(Self { buffer })
    }

    /// Returns the alignment requirement as a power of 2.
    ///
    /// The alignment requirement determines the granularity at which the read
    /// pointer can be advanced. For example, a return value of `3` means 8-byte
    /// alignment (2^3 = 8), requiring all read advances to be multiples of 8 bytes.
    pub fn alignment_pow2(&self) -> u8 {
        self.buffer.alignment_pow2
    }
}

impl Drop for MockNonAliasedBufferReader {
    /// Releases the read lock on the buffer when the reader is dropped.
    ///
    /// This allows a new reader to be created for the buffer after this one
    /// goes out of scope.
    fn drop(&mut self) {
        self.buffer.inner.lock().unwrap().read_locked = false;
    }
}

/// A writer for [`MockNonAliasedBuffer`] that implements [`CircularBufferWriter`].
///
/// Provides write access to the buffer's writable region and manages the write
/// pointer position. Unlike the aliased buffer, writable space may be fragmented into
/// two separate slices when it wraps around the buffer boundary.
///
/// Only one writer can exist for a buffer at a time. Attempting to create a second
/// writer will fail. The buffer is automatically unlocked when the writer is dropped.
///
/// # Safety
///
/// Contains a raw pointer to the underlying buffer. Ring buffers are generally external
/// to the program (DMA, RDMA, inter-process communications, etc.) so there is no way of
/// ensuring the buffer outlives the writer. It is the responsibility of the user to
/// ensure this.
pub struct MockNonAliasedBufferWriter {
    buffer: MockNonAliasedBuffer,
}

impl MockNonAliasedBufferWriter {
    /// Creates a new writer for the given buffer.
    ///
    /// Only one writer can exist for a buffer at a time. This constructor checks
    /// whether the buffer already has an active writer and fails if one exists.
    /// The writer lock is automatically set upon successful creation and released
    /// when the writer is dropped.
    ///
    /// # Arguments
    ///
    /// * `buffer` - Mutable reference to the buffer to write to
    ///
    /// # Returns
    ///
    /// * `Ok(Self)` - Successfully created writer with exclusive write access
    /// * `Err(&'static str)` - Buffer already has an active writer
    ///
    /// # Examples
    ///
    /// ```
    /// # use circular_buffer::mock_buffers::{MockNonAliasedBuffer, MockNonAliasedBufferWriter};
    /// #
    /// let mut buffer = MockNonAliasedBuffer::new(1024, 3).unwrap();
    /// let writer = MockNonAliasedBufferWriter::new(buffer.clone()).unwrap();
    ///
    /// // This will fail because a writer already exists
    /// assert!(MockNonAliasedBufferWriter::new(buffer.clone()).is_err());
    /// ```
    pub fn new(buffer: MockNonAliasedBuffer) -> Result<Self, &'static str> {
        let mut buffer_guard = buffer.inner.lock().unwrap();
        if buffer_guard.write_locked {
            return Err("Buffer already has a writer");
        }
        buffer_guard.write_locked = true;
        drop(buffer_guard);
        Ok(Self { buffer })
    }

    /// Returns the alignment requirement as a power of 2.
    ///
    /// The alignment requirement determines the granularity at which the write
    /// pointer can be advanced. For example, a return value of `3` means 8-byte
    /// alignment (2^3 = 8), requiring all write advances to be multiples of 8 bytes.
    pub fn alignment_pow2(&self) -> u8 {
        self.buffer.alignment_pow2
    }
}

impl Drop for MockNonAliasedBufferWriter {
    /// Releases the write lock on the buffer when the writer is dropped.
    ///
    /// This allows a new writer to be created for the buffer after this one
    /// goes out of scope.
    fn drop(&mut self) {
        self.buffer.inner.lock().unwrap().write_locked = false;
    }
}

/// Errors that can occur when advancing read or write pointers on the [`MockNonAliasedBuffer`].
#[derive(Debug, Error)]
pub enum NonAliasedAdvanceError {
    /// Attempted to advance beyond available data (reader) or space (writer).
    #[error("Not enough data available")]
    OutOfBounds,
    /// The requested advance amount does not satisfy alignment requirements.
    #[error("Result address not aligned")]
    NotAligned,
}

impl CircularBufferReader for MockNonAliasedBufferReader {
    type AdvanceStatus = ();
    type AdvanceError = NonAliasedAdvanceError;
    type ReadableRegion<'buf_ref> = (&'buf_ref [u8], &'buf_ref [u8]);
    type ReadableRegionError = Infallible;

    /// Advances the read pointer by the specified number of bytes.
    ///
    /// This method validates that the advance amount is properly aligned and that
    /// enough data is available before updating the read pointer. The read pointer
    /// automatically wraps around to the beginning when it reaches the buffer capacity.
    ///
    /// The `same_page` flag tracks whether the read and write pointers are on the
    /// same "lap" around the circular buffer, which is used to correctly calculate
    /// available data.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Number of bytes to advance. Must be aligned to the buffer's
    ///   alignment requirement and not exceed available data.
    ///
    /// # Errors
    ///
    /// * [`NonAliasedAdvanceError::NotAligned`] - The advance amount is not
    ///   properly aligned to the buffer's alignment requirement
    /// * [`NonAliasedAdvanceError::OutOfBounds`] - Not enough data available
    ///   to advance by the requested amount
    fn advance_read_pointer(
        &mut self,
        bytes: usize,
    ) -> Result<Self::AdvanceStatus, Self::AdvanceError> {
        // Check alignment
        if !ebutils::check_alignment_pow2(bytes, self.buffer.alignment_pow2) {
            return Err(NonAliasedAdvanceError::NotAligned);
        }

        // Check enough data available
        let (primary_region, secondary_region) = self.readable_region().unwrap();
        let available = primary_region.len() + secondary_region.len();
        if bytes > available {
            return Err(NonAliasedAdvanceError::OutOfBounds);
        }

        let mut buffer_guard = self.buffer.inner.lock().unwrap();

        // Handle wrapping when advancing
        if buffer_guard.read_ptr + bytes >= buffer_guard.buffer.len() {
            buffer_guard.read_ptr = (buffer_guard.read_ptr + bytes) % buffer_guard.buffer.len();
            buffer_guard.same_page = !buffer_guard.same_page;
        } else {
            buffer_guard.read_ptr += bytes;
        }

        Ok(())
    }

    /// Returns the readable data as a tuple of two slices.
    ///
    /// When readable data wraps around the buffer boundary, it is split into two slices:
    /// - **Primary slice**: Data from the current read position to either the write
    ///   position (same lap) or the end of the buffer (different laps)
    /// - **Secondary slice**: Data from the beginning of the buffer to the write position
    ///   (only when wrapping), or empty (when not wrapping)
    ///
    /// The amount of available data depends on whether the read and write pointers
    /// are on the same lap (`same_page`):
    /// - Same lap: primary = read_ptr to write_ptr, secondary = empty
    /// - Different laps: primary = read_ptr to end, secondary = start to write_ptr
    ///
    /// # Returns
    ///
    /// A tuple of `(primary_slice, secondary_slice)` containing all readable data.
    fn readable_region(&self) -> Result<Self::ReadableRegion<'_>, Infallible> {
        let buffer_guard = self.buffer.inner.lock().unwrap();

        if buffer_guard.same_page {
            // Primary: from read_ptr to write_ptr, Secondary: empty
            let primary_region =
                &buffer_guard.buffer[buffer_guard.read_ptr..buffer_guard.write_ptr];
            let primary_region =
                unsafe { &*slice_from_raw_parts(primary_region.as_ptr(), primary_region.len()) };
            Ok((primary_region, &[]))
        } else {
            // Primary: from read_ptr to end, Secondary: from start to write_ptr
            let primary_region = &buffer_guard.buffer[buffer_guard.read_ptr..];
            let primary_region =
                unsafe { &*slice_from_raw_parts(primary_region.as_ptr(), primary_region.len()) };
            let secondary_region = &buffer_guard.buffer[..buffer_guard.write_ptr];
            let secondary_region = unsafe {
                &*slice_from_raw_parts(secondary_region.as_ptr(), secondary_region.len())
            };
            Ok((primary_region, secondary_region))
        }
    }
}

impl CircularBufferWriter for MockNonAliasedBufferWriter {
    type AdvanceStatus = ();
    type AdvanceError = NonAliasedAdvanceError;
    type WriteableRegion<'buf_ref> = (&'buf_ref mut [u8], &'buf_ref mut [u8]);
    type WriteableRegionError = Infallible;

    /// Advances the write pointer by the specified number of bytes.
    ///
    /// This method validates that the advance amount is properly aligned and that
    /// enough space is available before updating the write pointer. The write pointer
    /// automatically wraps around to the beginning when it reaches the buffer capacity.
    ///
    /// The `same_page` flag tracks whether the read and write pointers are on the
    /// same "lap" around the circular buffer, which is used to correctly calculate
    /// available space.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Number of bytes to advance. Must be aligned to the buffer's
    ///   alignment requirement and not exceed available space.
    ///
    /// # Errors
    ///
    /// * [`NonAliasedAdvanceError::NotAligned`] - The advance amount is not
    ///   properly aligned to the buffer's alignment requirement
    /// * [`NonAliasedAdvanceError::OutOfBounds`] - Not enough space available
    ///   to advance by the requested amount
    fn advance_write_pointer(
        &mut self,
        bytes: usize,
    ) -> Result<Self::AdvanceStatus, Self::AdvanceError> {
        // Check alignment
        if !ebutils::check_alignment_pow2(bytes, self.buffer.alignment_pow2) {
            return Err(NonAliasedAdvanceError::NotAligned);
        }

        // Check enough data available
        let (primary_region, secondary_region) = self.writable_region().unwrap();
        let available = primary_region.len() + secondary_region.len();
        if bytes > available {
            return Err(NonAliasedAdvanceError::OutOfBounds);
        }

        let mut buffer_guard = self.buffer.inner.lock().unwrap();

        // Handle wrapping when advancing
        if buffer_guard.write_ptr + bytes >= buffer_guard.buffer.len() {
            buffer_guard.write_ptr = (buffer_guard.write_ptr + bytes) % buffer_guard.buffer.len();
            buffer_guard.same_page = !buffer_guard.same_page;
        } else {
            buffer_guard.write_ptr += bytes;
        }

        Ok(())
    }

    /// Returns the writable space as a tuple of two mutable slices.
    ///
    /// When writable space wraps around the buffer boundary, it is split into two slices:
    /// - **Primary slice**: Space from the current write position to either the read
    ///   position (different laps) or the end of the buffer (same lap)
    /// - **Secondary slice**: Space from the beginning of the buffer to the read position
    ///   (only when same lap), or empty (when different laps)
    ///
    /// The amount of available space depends on whether the read and write pointers
    /// are on the same lap (`same_page`):
    /// - Same lap: primary = write_ptr to end, secondary = start to read_ptr
    /// - Different laps: primary = write_ptr to read_ptr, secondary = empty
    ///
    /// # Returns
    ///
    /// A tuple of `(primary_slice, secondary_slice)` containing all writable space.
    fn writable_region(&mut self) -> Result<Self::WriteableRegion<'_>, Infallible> {
        let mut buffer_guard = self.buffer.inner.lock().unwrap();

        let buffer = unsafe {
            &mut *slice_from_raw_parts_mut(
                buffer_guard.buffer.as_mut_ptr(),
                buffer_guard.buffer.len(),
            )
        };

        if buffer_guard.same_page {
            // Primary: from write_ptr to end, Secondary: from start to read_ptr
            let (before_read, after_read) = buffer.split_at_mut(buffer_guard.read_ptr);
            let primary_region = &mut after_read[buffer_guard.write_ptr - buffer_guard.read_ptr..];
            Ok((primary_region, before_read))
        } else {
            // Primary: from write_ptr to read_ptr, Secondary: empty
            let primary_region = &mut buffer[buffer_guard.write_ptr..buffer_guard.read_ptr];
            Ok((primary_region, &mut []))
        }
    }
}
