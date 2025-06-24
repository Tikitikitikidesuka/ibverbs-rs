//! ## BufferElement Trait
//!
//! The `BufferElement` trait defines the interface that all elements written to the
//! shared memory ring buffer must implement. This trait provides the essential metadata
//! required for proper buffer management and ensures compatibility between producers
//! and consumers.

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
pub trait BufferElement {
    /// Returns the total size of the element in bytes.
    ///
    /// This includes all metadata and data that comprises the complete element.
    /// The implementation must store this information in a way that remains
    /// accessible even during partial writes.
    fn size(&self) -> usize;

    /// Returns information about the element's wrap status.
    ///
    /// **Note**: The return type suggests this may indicate wrap position or size
    /// rather than a simple boolean. Implementors should clarify the specific
    /// meaning for their element type.
    fn wrap(&self) -> usize;

    /// Marks the element as wrapping around the buffer boundary.
    ///
    /// This method is called by the writer when an element cannot fit in the
    /// remaining space at the end of the buffer. The wrap flag must be stored
    /// in the first two bytes to ensure durability.
    fn set_wrap(&mut self);

    /// Returns a byte slice containing the element's raw data.
    ///
    /// This provides access to the complete element as it appears in the buffer,
    /// including both metadata and payload data.
    fn data(&self) -> &[u8];
}