use crate::{CircularBufferReader, CircularBufferWriter};
use std::convert::Infallible;
use std::ptr::{slice_from_raw_parts, slice_from_raw_parts_mut};
use std::sync::{Arc, Mutex};
use thiserror::Error;

/// A mock implementation of an aliased single-producer single-consumer ring buffer.
///
/// An *aliased ring buffer* maps its data memory twice contiguously in virtual memory.
/// This allows reading a value that starts near the end of the buffer and wraps to the
/// beginning as if the memory were continuous, without the value needing to track its fragmentation.
/// In other words, it abstracts the responsibility for handling wrapping and fragmentation
/// from the values to the buffer itself.
/// Another way of abstracting this responsibility can be found in the [`MockNonAliasedBuffer`](crate::mock_buffers::MockNonAliasedBuffer)
///
/// Further reading on the topic can be found in [this blog post by Abhinav Agarwal](https://abhinavag.medium.com/a-fast-circular-ring-buffer-4d102ef4d4a3)
/// and [this blog post by Mike-Ash](https://www.mikeash.com/pyblog/friday-qa-2012-02-17-ring-buffers-and-mirrored-memory-part-ii.html)
/// which expands on triple aliased ring buffers to avoid equal value read and write pointer uncertainty.
///
/// For statically sized types, this is often unnecessary. A simpler approach is to make
/// the buffer length a multiple of the type’s size.
///
/// The [pcie40](../../pcie40) crate's _PCIe40 readout card_ buffer is an aliased spsc ring buffer.
#[derive(Debug, Clone)]
pub struct MockAliasedBuffer {
    inner: Arc<Mutex<MockAliasedBufferInner>>,
    alignment_pow2: u8,
}

#[derive(Debug, Clone)]
pub struct MockAliasedBufferInner {
    read_ptr: usize,
    write_ptr: usize,
    read_locked: bool,
    write_locked: bool,
    same_page: bool,
    buffer: Vec<u8>,
}

impl MockAliasedBuffer {
    /// Creates a new mock aliased buffer with the specified capacity and alignment.
    ///
    /// The buffer allocates double the requested capacity to simulate memory aliasing.
    /// The first half represents the "real" buffer, and the second half is the "aliased"
    /// region that mirrors the real buffer's contents. Since it's a mock buffer, no real
    /// aliasing will be set up and the memory will be manually mirrored by copying on
    /// every write.
    ///
    /// # Arguments
    ///
    /// * `capacity` - The logical capacity of the buffer in bytes. Must be aligned to
    ///   `2^alignment_pow2`. The actual allocation will be `capacity * 2`.
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
    /// # use circular_buffer::mock_buffers::MockAliasedBuffer;
    /// #
    /// // Create a 1024-byte buffer with 8-byte alignment
    /// let buffer = MockAliasedBuffer::new(1024, 3).unwrap();
    ///
    /// // This will fail because 1023 is not 8-byte aligned
    /// assert!(MockAliasedBuffer::new(1023, 3).is_err());
    /// ```
    pub fn new(capacity: usize, alignment_pow2: u8) -> Result<Self, &'static str> {
        if !ebutils::check_alignment_pow2(capacity, alignment_pow2) {
            Err("Capacity does not match alignment")
        } else {
            Ok(Self {
                inner: Arc::new(Mutex::new(MockAliasedBufferInner {
                    read_ptr: 0,
                    write_ptr: 0,
                    read_locked: false,
                    write_locked: false,
                    same_page: true,
                    buffer: vec![0; capacity * 2], // Double size for aliasing
                })),
                alignment_pow2,
            })
        }
    }

    /// Returns `true` if the buffer currently has an active reader.
    ///
    /// This flag is set when a [`MockAliasedBufferReader`] is created and cleared
    /// when it is dropped. It prevents multiple readers from being created simultaneously.
    pub fn read_locked(&self) -> bool {
        self.inner.lock().unwrap().read_locked
    }

    /// Returns `true` if the buffer currently has an active writer.
    ///
    /// This flag is set when a [`MockAliasedBufferWriter`] is created and cleared
    /// when it is dropped. It prevents multiple writers from being created simultaneously.
    pub fn write_locked(&self) -> bool {
        self.inner.lock().unwrap().write_locked
    }
}

impl MockAliasedBufferInner {
    /// Replicates written data to maintain the aliased memory illusion.
    ///
    /// After data is written to the buffer, this method ensures the "aliased" copy
    /// stays synchronized. This is critical for allowing contiguous reads across
    /// the wrap-around boundary without the reader needing to handle fragmentation.
    ///
    /// The method handles two cases:
    /// 1. **Write within real buffer**: Data written to the real buffer (first half)
    ///    is copied to the corresponding position in the aliased region (second half).
    /// 2. **Write crosses boundary**: When a write starts in the real buffer and
    ///    wraps into the aliased region, both directions are synchronized to maintain
    ///    consistency.
    ///
    /// # Arguments
    ///
    /// * `write_ptr` - Starting position where data was written (relative to buffer start)
    /// * `size` - Number of bytes that were written
    ///
    /// # Panics
    ///
    /// Will panic if `write_ptr + size` exceeds the total buffer length (capacity * 2),
    /// though this should be prevented by proper bounds checking in the writer.
    fn replicate_alias(&mut self, write_ptr: usize, size: usize) {
        let real_capacity = self.buffer.len() / 2;

        // If written to just real buffer
        if write_ptr + size < real_capacity {
            let (real_buffer, aliased_buffer) = self.buffer.split_at_mut(real_capacity);
            let written_region = &real_buffer[write_ptr..write_ptr + size];
            aliased_buffer[write_ptr..write_ptr + size].copy_from_slice(written_region);
        }
        // If write region crossed into aliased region
        else {
            let (real_buffer, aliased_buffer) = self.buffer.split_at_mut(real_capacity);

            // Copy real buffer written part to aliased buffer
            let written_region = &real_buffer[write_ptr..];
            aliased_buffer[write_ptr..].copy_from_slice(written_region);

            // Copy aliased buffer written part to real buffer
            let written_region = &aliased_buffer[..(write_ptr + size) % real_capacity];
            real_buffer[..(write_ptr + size) % real_capacity].copy_from_slice(written_region);
        }
    }
}

/// A reader for [`MockAliasedBuffer`] that implements [`CircularBufferReader`].
///
/// Provides read-only access to the buffer's readable region and manages the read
/// pointer position. The reader can access data contiguously across the wrap-around
/// boundary thanks to the buffer's aliased memory layout.
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
pub struct MockAliasedBufferReader {
    buffer: MockAliasedBuffer,
}

impl MockAliasedBufferReader {
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
    /// # use circular_buffer::mock_buffers::{MockAliasedBuffer, MockAliasedBufferReader};
    /// #
    /// let mut buffer = MockAliasedBuffer::new(1024, 3).unwrap();
    /// let reader = MockAliasedBufferReader::new(buffer.clone()).unwrap();
    ///
    /// // This will fail because a reader already exists
    /// assert!(MockAliasedBufferReader::new(buffer.clone()).is_err());
    /// ```
    pub fn new(buffer: MockAliasedBuffer) -> Result<Self, &'static str> {
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

impl Drop for MockAliasedBufferReader {
    /// Releases the read lock on the buffer when the reader is dropped.
    ///
    /// This allows a new reader to be created for the buffer after this one
    /// goes out of scope.
    fn drop(&mut self) {
        self.buffer.inner.lock().unwrap().read_locked = false;
    }
}

/// A writer for [`MockAliasedBuffer`] that implements [`CircularBufferWriter`].
///
/// Provides write access to the buffer's writable region and manages the write
/// pointer position. This mock writer maintains the aliased memory layout by automatically
/// replicating written data between the real and aliased regions when the write
/// pointer is advanced.
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
pub struct MockAliasedBufferWriter {
    buffer: MockAliasedBuffer,
}

impl MockAliasedBufferWriter {
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
    /// # use circular_buffer::mock_buffers::{MockAliasedBuffer, MockAliasedBufferWriter};
    /// #
    /// let mut buffer = MockAliasedBuffer::new(1024, 3).unwrap();
    /// let writer = MockAliasedBufferWriter::new(buffer.clone()).unwrap();
    ///
    /// // This will fail because a writer already exists
    /// assert!(MockAliasedBufferWriter::new(buffer.clone()).is_err());
    /// ```
    pub fn new(buffer: MockAliasedBuffer) -> Result<Self, &'static str> {
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

impl Drop for MockAliasedBufferWriter {
    /// Releases the write lock on the buffer when the writer is dropped.
    ///
    /// This allows a new writer to be created for the buffer after this one
    /// goes out of scope.
    fn drop(&mut self) {
        self.buffer.inner.lock().unwrap().write_locked = false;
    }
}

/// Errors that can occur when advancing read or write pointers on the [`MockAliasedBuffer`].
#[derive(Debug, Error)]
pub enum AliasedBufferAdvanceError {
    /// Attempted to advance beyond available data (reader) or space (writer).
    #[error("Not enough data available")]
    OutOfBounds,
    /// The requested advance amount does not satisfy alignment requirements.
    #[error("Result address not aligned")]
    NotAligned,
}

impl CircularBufferReader for MockAliasedBufferReader {
    type AdvanceStatus = ();
    type AdvanceError = AliasedBufferAdvanceError;
    type ReadableRegion<'buf_ref> = &'buf_ref [u8];
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
    /// * [`AliasedBufferAdvanceError::NotAligned`] - The advance amount is not
    ///   properly aligned to the buffer's alignment requirement
    /// * [`AliasedBufferAdvanceError::OutOfBounds`] - Not enough data available
    ///   to advance by the requested amount
    fn advance_read_pointer(
        &mut self,
        bytes: usize,
    ) -> Result<Self::AdvanceStatus, Self::AdvanceError> {
        let mut buffer_guard = self.buffer.inner.lock().unwrap();

        // Check alignment
        if !ebutils::check_alignment_pow2(bytes, self.buffer.alignment_pow2) {
            return Err(AliasedBufferAdvanceError::NotAligned);
        }

        // Check enough data available
        let available = buffer_guard.readable_region().len();
        if bytes > available {
            return Err(AliasedBufferAdvanceError::OutOfBounds);
        }

        // Handle wrapping when advancing
        let capacity = buffer_guard.buffer.len() / 2;
        if buffer_guard.read_ptr + bytes >= capacity {
            buffer_guard.read_ptr = (buffer_guard.read_ptr + bytes) % capacity;
            buffer_guard.same_page = !buffer_guard.same_page;
        } else {
            buffer_guard.read_ptr += bytes;
        }

        Ok(())
    }

    /// Returns a contiguous slice of all currently readable data.
    ///
    /// Thanks to the aliased memory layout, this always returns a single contiguous
    /// slice even when the readable data wraps around the buffer boundary. The slice
    /// extends from the current read pointer to include all data written by the writer
    /// but not yet consumed by the reader.
    ///
    /// The amount of available data depends on whether the read and write pointers
    /// are on the same lap (`same_page`):
    /// - Same lap: available = write_ptr - read_ptr
    /// - Different laps: available = (capacity - read_ptr) + write_ptr
    ///
    /// # Returns
    ///
    /// A slice containing all readable data starting at the current read position.
    fn readable_region(&self) -> Result<Self::ReadableRegion<'_>, Infallible> {
        let buffer_guard = self.buffer.inner.lock().unwrap();
        let readable_region = buffer_guard.readable_region();
        let readable_region =
            unsafe { &*slice_from_raw_parts(readable_region.as_ptr(), readable_region.len()) };
        Ok(readable_region)
    }
}

impl MockAliasedBufferInner {
    fn readable_region(&self) -> &[u8] {
        let available = if self.same_page {
            self.write_ptr - self.read_ptr
        } else {
            let capacity = self.buffer.len() / 2;
            capacity - self.read_ptr + self.write_ptr
        };

        let readable_region = &self.buffer[self.read_ptr..self.read_ptr + available];

        readable_region
    }
}

impl CircularBufferWriter for MockAliasedBufferWriter {
    type AdvanceStatus = ();
    type AdvanceError = AliasedBufferAdvanceError;
    type WriteableRegion<'buf_ref> = &'buf_ref mut [u8];
    type WriteableRegionError = Infallible;

    /// Advances the write pointer by the specified number of bytes.
    ///
    /// This method validates that the advance amount is properly aligned and that
    /// enough space is available before updating the write pointer. Critically, it
    /// replicates the written data to maintain the aliased memory illusion before
    /// advancing the pointer. The write pointer automatically wraps around to the
    /// beginning when it reaches the buffer capacity.
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
    /// * [`AliasedBufferAdvanceError::NotAligned`] - The advance amount is not
    ///   properly aligned to the buffer's alignment requirement
    /// * [`AliasedBufferAdvanceError::OutOfBounds`] - Not enough space available
    ///   to advance by the requested amount
    fn advance_write_pointer(
        &mut self,
        bytes: usize,
    ) -> Result<Self::AdvanceStatus, Self::AdvanceError> {
        let mut buffer_guard = self.buffer.inner.lock().unwrap();

        // Check alignment
        if !ebutils::check_alignment_pow2(bytes, self.buffer.alignment_pow2) {
            return Err(AliasedBufferAdvanceError::NotAligned);
        }

        // Check enough space available
        let available = buffer_guard.writable_region().len();
        if bytes > available {
            return Err(AliasedBufferAdvanceError::OutOfBounds);
        }

        // CRITICAL: Replicate the alias
        let write_ptr = buffer_guard.write_ptr;
        buffer_guard.replicate_alias(write_ptr, bytes);

        // Handle wrapping when advancing
        let capacity = buffer_guard.buffer.len() / 2;
        if buffer_guard.write_ptr + bytes >= capacity {
            buffer_guard.write_ptr = (buffer_guard.write_ptr + bytes) % capacity;
            buffer_guard.same_page = !buffer_guard.same_page;
        } else {
            buffer_guard.write_ptr += bytes;
        }

        Ok(())
    }

    /// Returns a mutable slice of all currently writable space.
    ///
    /// Thanks to the aliased memory layout, this always returns a single contiguous
    /// slice even when the writable space wraps around the buffer boundary. The slice
    /// extends from the current write pointer to include all space not occupied by
    /// unread data.
    ///
    /// The amount of available space depends on whether the read and write pointers
    /// are on the same lap (`same_page`):
    /// - Same lap: available = (capacity - write_ptr) + read_ptr
    /// - Different laps: available = read_ptr - write_ptr
    ///
    /// # Returns
    ///
    /// A mutable slice containing all writable space starting at the current write position.
    fn writable_region(&mut self) -> Result<Self::WriteableRegion<'_>, Infallible> {
        let mut buffer_guard = self.buffer.inner.lock().unwrap();
        let writable_region = buffer_guard.writable_region();
        let writable_region = unsafe {
            &mut *slice_from_raw_parts_mut(writable_region.as_mut_ptr(), writable_region.len())
        };
        Ok(writable_region)
    }
}

impl MockAliasedBufferInner {
    fn writable_region(&mut self) -> &mut [u8] {
        let available = if self.same_page {
            let capacity = self.buffer.len() / 2;
            capacity - self.write_ptr + self.read_ptr
        } else {
            self.read_ptr - self.write_ptr
        };

        &mut self.buffer[self.write_ptr..self.write_ptr + available]
    }
}
