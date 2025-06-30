extern crate core;

pub mod circular_buffer;
pub mod typed_circular_buffer;

#[cfg(feature = "multi-fragment-packet")]
pub mod multi_fragment_packet;

#[cfg(feature = "pcie40")]
pub mod pcie40;

#[cfg(feature = "mock-reader")]
pub mod mock_reader;

#[cfg(feature = "utils")]
pub mod utils;

#[cfg(feature = "shared-memory")]
pub mod shared_memory_buffer;
mod typed_circular_buffer_read_guard;
