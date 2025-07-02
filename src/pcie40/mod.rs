pub mod ctrl;
pub mod id;
pub mod reader;
pub mod stream;

mod bindings {
    // Suppress warnings about non-standard naming in imported C bindings and unused code
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(non_upper_case_globals)]
    #![allow(dead_code)]

    include!("bindings.rs");
}
