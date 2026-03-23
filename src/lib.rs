//! # ibverbs-rs
//!
//! Safe, ergonomic Rust bindings for the InfiniBand `libibverbs` API.
//!
//! This crate provides high-level abstractions for RDMA (Remote Direct Memory Access)
//! programming, built on top of the [`ibverbs-sys`](https://crates.io/crates/ibverbs-sys)
//! FFI bindings.
//!
//! ## Modules
//!
//! - [`ibverbs`] — Core RDMA primitives: device discovery, protection domains, queue pairs,
//!   memory registration, completion queues, and work requests.
//! - [`channel`] — A single point-to-point RDMA connection with a builder API and
//!   scope-based completion polling.
//! - [`multi_channel`] — Multiple parallel RDMA connections sharing a protection domain,
//!   with support for scatter/gather and one-sided operations.
//! - [`network`] — Distributed coordination: TCP-based endpoint exchange, network
//!   configuration, and barrier synchronization (linear, binary tree, and dissemination
//!   algorithms).
//!
//! ## Safety model
//!
//! Two-sided operations (send/receive) are lifetime-safe through Rust's borrow checker.
//! One-sided operations (RDMA read/write) require `unsafe` on the passive side, since the
//! remote peer can access registered memory at any time.
//!
//! See the [`ibverbs::memory`] module documentation for a detailed explanation of the
//! safety architecture.

pub mod channel;
pub mod ibverbs;
pub mod multi_channel;
pub mod network;
