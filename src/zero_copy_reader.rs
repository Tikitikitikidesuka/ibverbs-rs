//! # Zero-Copy Ring Buffer Reader
//!
//! A safe, zero-overhead abstraction for managing ring buffer access in DMA operations.
//!
//! ## Overview
//!
//! This module provides a robust interface for working with ring buffers in a zero-copy
//! manner, particularly useful for Direct Memory Access (DMA) operations. It leverages
//! Rust's ownership system to provide compile-time guarantees that prevent data races and
//! use-after-free errors.
//!
//! ## Core Components
//!
//! - `ZeroCopyReaderImpl`: Trait defining the core behavior implementers must provide
//! - `ZeroCopyRingBufferReader`: Safe wrapper that enforces the access safety contract
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
//! ## Example Usage
//! TODO: ADD EXAMPLE USAGE

use std::ops::Deref;

/// Safe wrapper around a zero-copy ring buffer reader implementation.
///
/// This struct enforces the safety contract that data references cannot
/// be invalidated while in use. It achieves this by returning a `DataGuard`
/// that ties the data lifetime to the reader's borrow.
pub trait ZeroCopyRingBufferReader {
    /// Returns a guarded reference to the current valid data.
    ///
    /// The returned `DataGuard` must borrow the reader, preventing any
    /// modification operations until the guard is dropped. This ensures
    /// the data reference remains valid for its entire lifetime.
    fn data(&self) -> DataGuard<Self>;

    /// Advances the write pointer, marking new data as available.
    ///
    /// Typically called after DMA hardware writes to the buffer.
    /// Cannot be called while a `DataGuard` from this reader exists.
    ///
    /// # Returns
    ///
    /// The number of bytes that were newly added
    fn load_data(&mut self, num_bytes: usize) -> usize;

    /// Advances the read pointer, marking data as processed.
    ///
    /// Cannot be called while a `DataGuard` from this reader exists.
    ///
    /// # Returns
    ///
    /// The number of bytes that were discarded
    fn discard_data(&mut self, num_bytes: usize) -> usize;
}

/// A guard that provides safe access to data from a `ZeroCopyRingBufferReader`.
///
/// This struct holds both a reference to the reader (preventing modification)
/// and a reference to the valid data. This ensures that the data reference
/// remains valid for the entire lifetime of the guard.
pub struct DataGuard<'a, R: ZeroCopyRingBufferReader + ?Sized> {
    data: &'a [u8],
    reader: &'a R,
}

impl<'a, R: ZeroCopyRingBufferReader> DataGuard<'a, R> {
    pub fn new(reader: &'a R, data: &'a [u8]) -> Self {
        DataGuard { data, reader }
    }
}

// Allow DataGuard to be used like a slice
impl<'a, R: ZeroCopyRingBufferReader> Deref for DataGuard<'a, R> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.data
    }
}
