mod non_aliased_buffer;
mod non_aliased_readable;
mod non_aliased_writable;

pub use non_aliased_buffer::*;
pub use non_aliased_readable::*;
pub use non_aliased_writable::*;

const VALID_MAGIC: [u8; 2] = [0xAA, 0xAA];
const WRAP_MAGIC: [u8; 2] = [0x55, 0x55];