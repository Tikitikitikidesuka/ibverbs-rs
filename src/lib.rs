extern crate core;

mod bindings {
    // Suppress warnings about non-standard naming in imported C bindings and unused code
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(non_upper_case_globals)]
    #![allow(dead_code)]

    include!("bindings.rs");
}

pub mod typed_zero_copy_ring_buffer_reader;
pub mod zero_copy_ring_buffer_reader;

#[cfg(feature = "multi_fragment_packet")]
pub mod multi_fragment_packet;

#[cfg(feature = "pcie40")]
pub mod pcie40;

pub mod utils;

pub mod mock_reader;
