extern crate core;

pub mod typed_zero_copy_ring_buffer_reader;
pub mod zero_copy_ring_buffer_reader;

#[cfg(feature = "multi_fragment_packet")]
pub mod multi_fragment_packet;

#[cfg(feature = "pcie40")]
pub mod pcie40;

pub mod utils;

pub mod mock_reader;
