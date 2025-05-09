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
//! - `ZeroCopyRingBufferReader`: Safe trait that enforces the access safety contract
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

use std::cmp::min;
use std::fmt::Debug;
use std::ops::Deref;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ZeroCopyRingBufferReaderError {
    #[error("Connection to the ring buffer failed: {0}")]
    ConnectionError(String),
}

/// Safe wrapper around a zero-copy ring buffer reader implementation.
///
/// This struct enforces the safety contract that data references cannot
/// be invalidated while in use. It achieves this by returning a `DataGuard`
/// that ties the data lifetime to the reader's borrow.
pub trait ZeroCopyRingBufferReader {
    // TODO: DOCUMENT
    // This is here to make users implement a method to access the data since a reference to it cannot be
    // held inside of the guard. That was done before but then the guard cannot hold a mutable to the reader
    // easily. CHECK THIS COMMIT AND THE PREVIOUS ONE OUT BECAUSE BIG CHANGES DUE TO THE MUT REQUIREMENT FOR
    // THE GUARD DISCARD METHOD
    // This is unsafe because the reader is not locked while the data reference exists, data of this reference could
    // be discarded while holding the reference and therefore be invalid. When guarded by a DataGuard this is solved.
    unsafe fn unsafe_data(&self) -> &[u8];

    /// Returns a guarded reference to the current valid data.
    ///
    /// The returned `DataGuard` must borrow the reader, preventing any
    /// modification operations until the guard is dropped. This ensures
    /// the data reference remains valid for its entire lifetime.
    fn data(&mut self) -> DataGuard<Self> {
        DataGuard::new(self)
    }

    /// Reads the write pointer, updating the available data to read.
    ///
    /// Cannot be called while a `DataGuard` from this reader exists.
    ///
    /// # Returns
    ///
    /// The number of extra bytes loaded (write pointer - previous write pointer)
    fn load_all_data(&mut self) -> Result<usize, ZeroCopyRingBufferReaderError>;

    /// Advances the read pointer num_bytes or until write pointer, marking data as processed.
    ///
    /// Cannot be called while a `DataGuard` from this reader exists.
    ///
    /// # Returns
    ///
    /// The number of bytes that were discarded
    fn discard_data(&mut self, num_bytes: usize) -> Result<usize, ZeroCopyRingBufferReaderError>;

    /// Advances the read pointer until the write pointer, marking data as processed.
    ///
    /// Cannot be called while a `DataGuard` from this reader exists.
    ///
    /// # Returns
    ///
    /// The number of bytes that were discarded
    fn discard_all_data(&mut self) -> Result<usize, ZeroCopyRingBufferReaderError>;

    /// Returns the alignment of the ring buffer if there is Some(alignment_bytes)
    /// Defaults to having no alignment
    fn alignment(&self) -> Result<Option<usize>, ZeroCopyRingBufferReaderError> {
        Ok(None)
    }
}

/// A guard that provides safe access to data from a `ZeroCopyRingBufferReader`.
///
/// This struct holds both a reference to the reader (preventing modification)
/// and a reference to the valid data. This ensures that the data reference
/// remains valid for the entire lifetime of the guard.
pub struct DataGuard<'a, R: ZeroCopyRingBufferReader + ?Sized> {
    reader: &'a mut R,
}

// TODO: DOCUMENT
impl<'a, R: ZeroCopyRingBufferReader + ?Sized> DataGuard<'a, R> {
    pub fn new(reader: &'a mut R) -> Self {
        DataGuard { reader }
    }

    pub fn discard(self) -> Result<(), ZeroCopyRingBufferReaderError> {
        let data_length = unsafe { self.reader.unsafe_data().len() };
        let discarded_length = self.reader.discard_data(data_length)?;

        if data_length != discarded_length {
            unreachable!(
                "When calling `discard` on a DataGuard its data is supposed to be guaranteed on the buffer...\n\
                This error implies an erroneous implementation of the reader because {data_length}\
                were supposed to be discarded but only {discarded_length} were available to discard.\n\
                Please contact the developers for further assistance."
            );
        }

        Ok(())
    }

    // Guarantees discarding at num_bytes if at least num_bytes are guarded or the whole data guard otherwise
    pub fn discard_count(self, num_bytes: usize) -> Result<usize, ZeroCopyRingBufferReaderError> {
        let guarded_length = unsafe { self.reader.unsafe_data().len() };
        let expected_discarded_length = min(guarded_length, num_bytes);
        let discarded_length = self.reader.discard_data(expected_discarded_length)?;

        if discarded_length != expected_discarded_length {
            unreachable!(
                "When calling `discard_count` on a DataGuard its data is supposed to be guaranteed on the buffer...\n\
                This error implies an erroneous implementation of the reader because {expected_discarded_length} \
                were supposed to be discarded but only {discarded_length} were available to discard.\n\
                Please contact the developers for further assistance."
            );
        }

        Ok(discarded_length)
    }

    pub fn data_ref(&self) -> &[u8] {
        unsafe { self.reader.unsafe_data() }
    }

    pub fn reader_ref(&self) -> &R {
        self.reader
    }
}

// TODO: DOCUMENT
// Allow DataGuard to be used like a slice
impl<R: ZeroCopyRingBufferReader + ?Sized> Deref for DataGuard<'_, R> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.data_ref()
    }
}

impl<R: ZeroCopyRingBufferReader + ?Sized> Debug for DataGuard<'_, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        const MAX_PREVIEW_BYTES: usize = 16;

        let data = self.data_ref();
        let data_len = data.len();

        let preview_data = if data_len <= MAX_PREVIEW_BYTES {
            data
        } else {
            &data[..MAX_PREVIEW_BYTES]
        };

        write!(f, "DataGuard {{ data: [")?;

        for (i, byte) in preview_data.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{:#04x}", byte)?;
        }

        if data_len > MAX_PREVIEW_BYTES {
            write!(f, ", ... ({} more bytes)", data_len - MAX_PREVIEW_BYTES)?;
        }

        write!(f, "] }}")
    }
}
