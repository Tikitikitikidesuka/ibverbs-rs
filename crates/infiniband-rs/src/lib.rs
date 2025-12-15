pub mod context;
pub mod devices;

// TODO: This modules will become private after connection type is implemented
pub mod protection_domain;
pub mod completion_queue;
pub mod connection;
mod global_id;
mod global_unique_id;
mod memory_region;
pub mod network;
mod prepared_queue_pair;
mod queue_pair;
mod queue_pair_endpoint;
pub mod unsafe_member;

pub mod queue_pair_builder;