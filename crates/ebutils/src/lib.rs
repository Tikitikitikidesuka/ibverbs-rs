pub mod alignment;
pub mod fragment;
pub mod fragment_type;
pub mod odin;
pub mod source_id;

pub use alignment::*;

/// Zero sized marker type that cannot be instantiated.
/// To be replaced by an external type later on, see https://github.com/rust-lang/rust/issues/43467.
pub struct Uninstantiatable(());

/// Type of a source id.
pub type EventId = u64;
