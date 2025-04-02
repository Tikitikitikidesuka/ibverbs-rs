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

use std::ops::Deref;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ZeroCopyRingBufferReaderError {
    #[error("Connection to the ring buffer failed: {0}")]
    ConnectionError(String),
}

#[derive(Debug, Error)]
pub enum ZeroCopyRingBufferReaderTypedReadError {
    #[error("{0}")]
    ZeroCopyRingBufferReaderError(ZeroCopyRingBufferReaderError),

    #[error(
        "Missing data: Type is {type_size} bytes long. Only {available_data} bytes available in the buffer"
    )]
    MissingData {
        type_size: usize,
        available_data: usize,
    },
}

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

    /// Advances the loaded data pointer num_bytes or until the write pointer if reached,
    /// marking new data as available.
    ///
    /// Typically called after DMA hardware writes to the buffer.
    /// Cannot be called while a `DataGuard` from this reader exists.
    ///
    /// # Returns
    ///
    /// The number of bytes that were newly added
    fn load_data(&mut self, num_bytes: usize) -> Result<usize, ZeroCopyRingBufferReaderError>;

    /// Advances the loaded data pointer until the write pointer, marking new data as available.
    ///
    /// Typically called after DMA hardware writes to the buffer.
    /// Cannot be called while a `DataGuard` from this reader exists.
    ///
    /// # Returns
    ///
    /// The number of bytes that were newly added
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

    // TODO: DOCUMENT
    fn typed_read<T: ZeroCopyRingBufferReadable<Self>>(
        &mut self,
    ) -> Result<TypedDataGuard<Self, T>, ZeroCopyRingBufferReaderTypedReadError> {
        let typed_data = T::read(self)?;
        Ok(TypedDataGuard::new(self, typed_data))
    }

    /*
    // TODO: DOCUMENT
    fn read_multiple<T: ZeroCopyRingBufferReadable>(
        &mut self,
        num: usize,
    ) -> Result<TypedDataGuard<Self, Vec<T>>, ZeroCopyRingBufferReaderError> {
        Ok((0..num).into_iter().map(|_| self.read()).collect())
    }
    */
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

// TODO: DOCUMENT
impl<'a, R: ZeroCopyRingBufferReader + ?Sized> DataGuard<'a, R> {
    pub fn new(reader: &'a R, data: &'a [u8]) -> Self {
        DataGuard { data, reader }
    }
}

// TODO: DOCUMENT
// Allow DataGuard to be used like a slice
impl<'a, R: ZeroCopyRingBufferReader + ?Sized> Deref for DataGuard<'a, R> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

// TODO: DOCUMENT
pub struct TypedDataGuard<'a, R: ZeroCopyRingBufferReader + ?Sized, T: 'a> {
    typed_data: &'a T,
    reader: &'a R,
}

// TODO: DOCUMENT
impl<'a, R: ZeroCopyRingBufferReader + ?Sized, T: 'a> TypedDataGuard<'a, R, T> {
    pub fn new(reader: &'a R, typed_data: &'a T) -> Self {
        TypedDataGuard { typed_data, reader }
    }
}

// TODO: DOCUMENT
impl<'a, R: ZeroCopyRingBufferReader + ?Sized, T: 'a> Deref for TypedDataGuard<'a, R, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.typed_data
    }
}

// TODO: DOCUMENT
pub trait ZeroCopyRingBufferReadable<R: ZeroCopyRingBufferReader + ?Sized> {
    // TODO: DOCUMENT
    fn read<'a>(buffer: &mut R) -> Result<&'a Self, ZeroCopyRingBufferReaderTypedReadError>;
}
