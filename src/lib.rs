extern crate core;

mod bindings {
    // Suppress warnings about non-standard naming in imported C bindings and unused code
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(non_upper_case_globals)]
    #![allow(dead_code)]

    include!("bindings.rs");
}

pub mod zero_copy_ring_buffer_reader;

pub mod multi_fragment_packet;
pub mod pcie40;
pub mod typed_zero_copy_ring_buffer_reader;
pub mod utils;
