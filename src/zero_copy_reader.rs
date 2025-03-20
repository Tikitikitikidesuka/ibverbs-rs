//! # Zero-Copy Reader
//!
//! A safe, zero-overhead abstraction for managing buffer access in DMA operations.
//!
//! ## Overview
//!
//! This module provides a robust interface for working with memory buffers in a zero-copy
//! manner, particularly useful for Direct Memory Access (DMA) operations. It leverages
//! Rust's ownership system to provide compile-time guarantees that prevent data races and
//! use-after-free errors.
//!
//! ## Core Components
//!
//! - `ZeroCopyReaderImpl`: Trait defining the core behavior implementers must provide
//! - `ZeroCopyReader`: Safe wrapper that enforces the access safety contract
//! - `DataGuard`: Handle that protects data references from invalidation
//!
//! ## Safety Guarantees
//!
//! This design ensures:
//!
//! 1. Data references cannot be invalidated while in use
//! 2. Buffer pointers cannot be modified while data is being accessed
//! 3. All safety checks occur at compile time with zero runtime overhead
//!
//! ## Usage Example
//!
//! ```
//! // Create a reader with your implementation
//! # use pcie40_rs::zero_copy_reader::{ZeroCopyReader, ZeroCopyReaderImpl};
//! #
//! # struct MyReaderImpl {
//! #     buffer: Vec<u8>,
//! #     start: usize,
//! #     end: usize,
//! # }
//! #
//! # impl MyReaderImpl {
//! #     fn new() -> Self {
//! #         let mut buffer = Vec::with_capacity(1024);
//! #         buffer.extend(0..100u8);
//! #         Self { buffer, start: 0, end: 0 }
//! #     }
//! # }
//! #
//! # impl ZeroCopyReaderImpl for MyReaderImpl {
//! #     fn data(&self) -> &[u8] {
//! #         &self.buffer[self.start..self.end]
//! #     }
//! #
//! #     fn load_data(&mut self, num_bytes: usize) -> usize {
//! #         let new_end = std::cmp::min(self.end + num_bytes, self.buffer.len());
//! #         let added = new_end - self.end;
//! #         self.end = new_end;
//! #         added
//! #     }
//! #
//! #     fn discard_data(&mut self, num_bytes: usize) -> usize {
//! #         let new_start = std::cmp::min(self.start + num_bytes, self.end);
//! #         let discarded = new_start - self.start;
//! #         self.start = new_start;
//! #         discarded
//! #     }
//! # }
//! #
//!
//! let mut reader = ZeroCopyReader::new(MyReaderImpl::new());
//!
//! // Simulate data being received via DMA
//! reader.load_data(32);
//!
//! // Safely access the data
//! let guard = reader.data();
//! println!("{:?}", &guard[0..16]);
//!
//! // Won't compile: reader.discard_data(16);
//!
//! // Must drop the guard first
//! drop(guard);
//! reader.discard_data(16);
//! ```

use std::ops::Deref;

/// Trait defining the core operations for zero-copy buffer access.
///
/// Implementers of this trait provide the low-level functionality for
/// accessing and manipulating buffer data. The safety guarantees are
/// enforced by the `ZeroCopyReader` wrapper.
pub trait ZeroCopyReaderImpl {
    /// Returns a reference to the current valid data in the buffer.
    fn data(&self) -> &[u8];

    /// Advances the write pointer, marking new data as available.
    ///
    /// Typically called after DMA hardware writes data to the buffer.
    ///
    /// # Returns
    ///
    /// The number of bytes that were newly added
    fn load_data(&mut self, num_bytes: usize) -> usize;

    /// Advances the read pointer, marking data as processed.
    ///
    /// # Returns
    ///
    /// The number of bytes that were discarded
    fn discard_data(&mut self, num_bytes: usize) -> usize;
}

/// Safe wrapper around a zero-copy reader implementation.
///
/// This struct enforces the safety contract that data references cannot
/// be invalidated while in use. It achieves this by returning a `DataGuard`
/// that ties the data lifetime to the reader's borrow.
pub struct ZeroCopyReader<R: ZeroCopyReaderImpl> {
    reader_impl: R,
}

impl<R: ZeroCopyReaderImpl> ZeroCopyReader<R> {
    /// Creates a new ZeroCopyReader with the provided implementation.
    pub fn new(reader_impl: R) -> Self {
        Self { reader_impl }
    }

    /// Returns a guarded reference to the current valid data.
    ///
    /// The returned `DataGuard` borrows the reader, preventing any
    /// modification operations until the guard is dropped. This ensures
    /// the data reference remains valid for its entire lifetime.
    pub fn data(&self) -> DataGuard<'_, R> {
        DataGuard {
            data: self.reader_impl.data(),
            reader: &self.reader_impl,
        }
    }

    /// Advances the write pointer, marking new data as available.
    ///
    /// Typically called after DMA hardware writes to the buffer.
    /// Cannot be called while a `DataGuard` from this reader exists.
    ///
    /// # Returns
    ///
    /// The number of bytes that were newly added
    pub fn load_data(&mut self, num_bytes: usize) -> usize {
        self.reader_impl.load_data(num_bytes)
    }

    /// Advances the read pointer, marking data as processed.
    ///
    /// Cannot be called while a `DataGuard` from this reader exists.
    ///
    /// # Returns
    ///
    /// The number of bytes that were discarded
    pub fn discard_data(&mut self, num_bytes: usize) -> usize {
        self.reader_impl.discard_data(num_bytes)
    }
}

/// A guard that provides safe access to data from a `ZeroCopyReader`.
///
/// This struct holds both a reference to the reader (preventing modification)
/// and a reference to the valid data. This ensures that the data reference
/// remains valid for the entire lifetime of the guard.
pub struct DataGuard<'a, R: ZeroCopyReaderImpl> {
    data: &'a [u8],
    reader: &'a R,
}

impl<'a, R: ZeroCopyReaderImpl> DataGuard<'a, R> {
    /// Returns a reference to the valid data.
    pub fn data(&self) -> &[u8] {
        self.data
    }
}

// Allow DataGuard to be used like a slice
impl<'a, R: ZeroCopyReaderImpl> Deref for DataGuard<'a, R> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.data
    }
}