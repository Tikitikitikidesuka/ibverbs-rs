//! ## BufferElement Trait
//!
//! The `BufferElement` trait defines the interface that all elements written to the
//! shared memory ring buffer must implement. This trait provides the essential metadata
//! required for proper buffer management and ensures compatibility between producers
//! and consumers.

use crate::readable_buffer_element::SharedMemoryTypedReadError;
use crate::writable_buffer_element::SharedMemoryTypedWriteError;

/// A trait for elements that can be stored in the shared memory ring buffer.
///
/// Implementors must provide access to the three critical pieces of metadata:
/// size, validity, and wrap status. The implementation is responsible for how
/// this metadata is stored and represented within the element's data structure.
///
/// ## Wrap Flag Placement
///
/// **Critical**: The wrap flag must be stored in the first two bytes of the element
/// to ensure durability during partial writes at buffer boundaries. Since the buffer
/// maintains minimum 2-byte alignment, placing the wrap flag in the first two bytes
/// guarantees it will be written and readable even when an element crosses the
/// wraparound boundary.

pub trait SharedMemoryBufferElement {
    /// Returns the element's size in bytes.
    fn length_in_bytes(&self) -> usize;
}

pub trait ReadableSharedMemoryBufferElement: SharedMemoryBufferElement {
    /// Returns a reference to a `Self` if possible to cast from the provided raw data.
    /// Otherwise, a `SharedMemoryTypedReadError` is raised.
    fn cast_to_element(data: &[u8]) -> Result<&Self, SharedMemoryTypedReadError>;

    /// Returns the wrap flag state for this element. If the element wraps, meaning it would reach
    /// the end of the buffer, and therefore it must be written to the beginning, the flag is true.
    ///
    /// As a parameter it should receive the byte slice with the beginning of the element.
    /// If there are not enough bytes for the wrap flag or any other occurs, a `SharedMemoryTypedReadError`
    /// should be returned.
    ///
    /// The fewer bytes we need for the wrap flag, the better.
    /// It's desirable to store the wrap flag in the first two bytes since the
    /// minimum alignment of the buffer is two bytes because of how it stores pointers. Meaning, if
    /// the wrap flag is in the first two bytes, there will always be space for it.
    fn check_wrap_flag(bytes: &[u8]) -> Result<bool, SharedMemoryTypedReadError>;
}

pub trait WritableSharedMemoryBufferElement: SharedMemoryBufferElement {
    /// Writes the element to memory as raw bytes.
    fn write_to_buffer(&self, buffer: &mut [u8]) -> Result<(), SharedMemoryTypedWriteError>;

    /// Sets the wrap flag as if the element started on the first byte of `bytes`.
    ///
    /// The writer calls this method when an element cannot fit in the
    /// remaining space at the end of the buffer. The wrap flag must be stored
    /// in the first two bytes to ensure durability.
    fn set_wrap_flag(bytes: &mut [u8]) -> Result<(), SharedMemoryTypedWriteError>;
}