pub mod pcie40_ctrl;
pub mod pcie40_id;
pub mod pcie40_stream;
pub mod pcie40_reader;

mod bindings {
    // Suppress warnings about non-standard naming in imported C bindings and unused code
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(non_upper_case_globals)]
    #![allow(dead_code)]

    include!("bindings.rs");
}
