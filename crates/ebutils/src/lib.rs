#![doc = include_str!("../README.md")]

pub mod alignment;
pub mod fragment;
pub mod fragment_type;
pub mod odin;
pub mod source_id;

pub use alignment::*;

/// Zero sized marker type that cannot be instantiated.
/// To be replaced by an external type later on, see <https://github.com/rust-lang/rust/issues/43467>.
pub struct Uninstantiatable(());

/// An identifier uniquely identifying an "event" within a run.
pub type EventId = u64;

pub use fragment::Fragment;
pub use fragment_type::FragmentType;
pub use odin::OdinPayload;
pub use source_id::SourceId;
pub use source_id::SubDetector;

