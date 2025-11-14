//! Mock buffers, readers and writers to exemplify and test the interfaces defined by the crate.
//! Available with the `mock-buffers` feature.

mod aliased;
mod dynamic_size_element;
mod non_aliased;

pub use aliased::*;
pub use dynamic_size_element::*;
pub use non_aliased::*;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReadError {
    #[error("Type not found on buffer")]
    NotFound,

    #[error("Not enough data for requested type")]
    NotEnoughData,

    #[error("Data is corrupt for requested type")]
    CorruptData,
}

#[derive(Debug, Error)]
pub enum WriteError {
    #[error("Not enough space for requested type")]
    NotEnoughSpace,
}
