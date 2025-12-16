// TODO: This modules will become private after connection type is implemented
pub mod connection;

mod ibverbs;
pub use ibverbs::context;
pub use ibverbs::devices;

mod unsafe_member;