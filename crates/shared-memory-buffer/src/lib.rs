//! # Shared Memory Ring Buffer
//!
//! An inter process communication mechanism using Unix `fcntl` file locks and POSIX `shmem`
//! shared memory to facilitate data exchange between processes through a ring buffer.
//! It operates in a single-reader, single-writer communication model.
//!
//! ## Buffer Structure
//!
//! The buffer is allocated as a POSIX `shmem` shared memory region with the following
//! characteristics:
//!
//! - All data is aligned to a constant power of two (typically the system page size)
//! - A header section contains metadata for buffer management
//! - Padding follows the header to maintain alignment
//! - The data exchange region occupies the remainder of the buffer
//!
//! ### Memory Layout
//!
//! ```text
//! +-----------------------------+------------------------------+
//! |   Buffer Header + Padding   |     Data Exchange Region     |
//! +-----------------------------+------------------------------+
//! ```
//!
//! ## Header Layout
//!
//! ```text
//! | Field             | Type     | Description              |
//! |-------------------|----------|--------------------------|
//! | `write_status`    | `u64`    | Current write status     |
//! | `read_status`     | `u64`    | Current read status      |
//! | `size`            | `usize`  | Total buffer size        |
//! | `alignment_pow2`  | `usize`  | Alignment as power of 2  |
//! | `id`              | `c_int`  | Buffer identifier        |
//! ```
//!
//! **NOTE:** This protocol is defined by the Online Event Builder C++ implementation.
//! Types are chosen specifically for compatibility with the C++ codebase.
//!
//! ### Write and Read Statuses
//!
//! The `write_status` and `read_status` are 64-bit values that encode both pointer position
//! and wraparound state:
//!
//! - **Bit 63**: Wraparound bit, flipped each time the pointer wraps to buffer start.
//! - **Bits 62-0**: Pointer position within the data exchange region.
//!
//! The wraparound bit distinguishes between two critical states when pointers are equal:
//! - **Empty Buffer**: Read and write pointers at same position, same wraparound bit
//! - **Full Buffer**: Read and write pointers at same position, different wraparound bits
//!
//! ## Protocol
//!
//! ### Ring Buffer
//!
//! The ring buffer uses two pointers to manage data flow:
//! - **Write Pointer**: Marks where the next write operation should begin.
//! - **Read Pointer**: Marks where the next read operation should begin.
//!
//! #### Valid Regions
//!
//! At any given time, the buffer is divided into two regions:
//! - **Readable Region**: From read pointer to write pointer (contains unread data).
//! - **Writable Region**: From write pointer to read pointer (available for new data).
//!
//! These regions may wrap around the buffer boundaries. For example, if the write pointer
//! is positioned before the read pointer in memory, the readable region spans from the
//! read pointer to the end of the buffer, then continues from the beginning to the write pointer.
//!
//! #### Operation Flow
//!
//! 1. **Writer Process**:
//!    - Writes data starting at the write pointer.
//!    - Advances write pointer by the amount of data written.
//!    - Flips wraparound bit when write pointer wraps to buffer start.
//!
//! 2. **Reader Process**:
//!    - Reads data starting at the read pointer.
//!    - Advances read pointer by the amount of data consumed.
//!    - Flips wraparound bit when read pointer wraps to buffer start.
//!
//! ### Element Abstraction
//!
//! All data written to this buffer must be structured as elements. Elements do not
//! necessarily share a common representation or prefix, but they must implement a
//! consistent interface that allows both producer and consumer to extract essential
//! metadata for buffer operations.
//!
//! Each element must provide three pieces of metadata:
//! - **Size**: The length in bytes of the element's data
//! - **Validity**: Whether the element's data is complete and valid
//! - **Wrap Status**: Whether the element crosses the buffer's wraparound boundary
//!
//! #### Reading Elements
//!
//! The reader uses this metadata to determine data boundaries and validity before
//! consuming the element. The buffer enforces a key constraint: **elements that wrap
//! around the buffer boundary cannot be read directly**.
//!
//! #### Handling Wrapped Elements
//!
//! When an element would cross the wraparound boundary, the writer must:
//! 1. Write the element with the wrap flag set, filling space up to the buffer's end
//! 2. If space is available, rewrite the complete element at the buffer's beginning
//!    without the wrap flag
//!
//! This ensures readers always encounter complete, non-wrapping elements.
//!
//! #### Wrap Flag Durability
//!
//! The wrap flag must be positioned so it remains readable even when an element is
//! partially written due to wraparound. If the wrap flag were located after the buffer's
//! end, it would not be written, leaving readers unable to detect the wrap condition.
//!
//! **Solution**: Place the wrap flag in the first two bytes of the element. Since the
//! buffer maintains minimum 2-byte alignment, these bytes are guaranteed to be written
//! whenever an element begins at the wraparound boundary.
//!
//! **Example**: In the MultiFragmentPacket implementation, the wrap flag is stored in
//! the first two bytes (magic field), ensuring it's always available to readers even
//! during wraparound scenarios.

mod buffer_backend;
mod buffer_element;
mod buffer_status;
mod file_lock;
mod readable_buffer_element;
mod reader;
mod shared_memory;
mod writable_buffer_element;
mod writer;

pub use buffer_backend::{
    SharedMemoryBuffer, SharedMemoryBufferNewError, SharedMemoryReadBuffer, SharedMemoryWriteBuffer,
};
pub use buffer_element::{
    ReadableSharedMemoryBufferElement, SharedMemoryBufferElement, WritableSharedMemoryBufferElement,
};
pub use circular_buffer::*;
pub use readable_buffer_element::SharedMemoryTypedReadError;
pub use reader::{SharedMemoryBufferAdvanceError, SharedMemoryBufferReader};
pub use writable_buffer_element::SharedMemoryTypedWriteError;
pub use writer::SharedMemoryBufferWriter;
