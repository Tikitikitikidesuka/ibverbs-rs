extern crate core;

mod bindings {
    // Suppress warnings about non-standard naming in imported C bindings and unused code
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(non_upper_case_globals)]
    #![allow(dead_code)]

    include!("bindings.rs");
}

pub mod pcie40_id;
pub mod pcie40_ctrl;
pub mod pcie40_stream;

pub mod zero_copy_ring_buffer_reader;
pub mod pcie40_reader;

pub mod test_readable;
pub mod typed_zero_copy_ring_buffer_reader;
pub mod demo_reader;
pub mod multi_fragment_packet;
pub mod utils;
