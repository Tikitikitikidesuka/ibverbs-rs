mod aliased_buffer;
mod aliased_readable;
mod aliased_writable;

pub use aliased_buffer::*;
pub use aliased_readable::*;
pub use aliased_writable::*;

const VALID_MAGIC: [u8; 2] = [0xAA, 0xAA];
