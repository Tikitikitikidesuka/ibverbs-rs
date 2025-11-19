//! Mock buffers, readers and writers to exemplify and test the interfaces defined by the crate.
//! Available with the `mock-buffers` feature.

//mod aliased;
mod dynamic_size_element;
mod non_aliased;

//pub use aliased::*;
pub use dynamic_size_element::*;
pub use non_aliased::*;
use thiserror::Error;

/// Errors that can occur when reading from the mock circular buffers.
#[derive(Debug, Error)]
pub enum ReadError {
    /// The requested type or entry was not found in the buffer.
    #[error("Type not found on buffer")]
    NotFound,

    /// Insufficient data available to complete the read operation.
    ///
    /// This can occur when there is not enough data for the header of the type or when
    /// the header specifies a length that is not available to read.
    #[error("Not enough data for requested type")]
    NotEnoughData,

    /// Data validation failed, indicating buffer corruption.
    #[error("Data is corrupt for requested type")]
    CorruptData,
}

/// Errors that can occur when writing to circular buffers.
#[derive(Debug, Error)]
pub enum WriteError {
    /// Insufficient space available to complete the write operation.
    #[error("Not enough space for requested type")]
    NotEnoughSpace,
}
