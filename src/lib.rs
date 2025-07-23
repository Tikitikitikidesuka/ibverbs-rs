extern crate core;

#[cfg(feature = "shared-memory")]
pub mod shared_memory_buffer;

#[cfg(feature = "mock-buffers")]
pub mod mock_buffers;
