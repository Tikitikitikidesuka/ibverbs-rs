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
//! | `alignment_2pow`  | `usize`  | Alignment as power of 2  |
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
//! The ring buffer uses two pointers to manage data flow:
//! - **Write Pointer**: Marks where the next write operation should begin.
//! - **Read Pointer**: Marks where the next read operation should begin.
//!
//! ### Valid Regions
//!
//! At any given time, the buffer is divided into two regions:
//! - **Readable Region**: From read pointer to write pointer (contains unread data).
//! - **Writable Region**: From write pointer to read pointer (available for new data).
//!
//! These regions may wrap around the buffer boundaries. For example, if the write pointer
//! is positioned before the read pointer in memory, the readable region spans from the
//! read pointer to the end of the buffer, then continues from the beginning to the write pointer.
//!
//! ### Operation Flow
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

pub mod buffer_backend;
mod buffer_status;
pub mod file_lock;
pub mod reader;
pub mod shared_memory;
