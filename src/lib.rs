//! # ibverbs-rs
//!
//! Safe, ergonomic Rust bindings for the InfiniBand `libibverbs` API.
//!
//! This crate provides high-level abstractions for RDMA (Remote Direct Memory Access)
//! programming, built on top of the [`ibverbs-sys`](https://crates.io/crates/ibverbs-sys)
//! FFI bindings.
//!
//! ## Getting started
//!
//! Choose the abstraction level that fits your use case:
//!
//! * **Single connection** — start with [`channel::Channel`]. It wraps a queue pair with
//!   lifetime-safe operation posting and scope-based completion polling.
//! * **Multiple peers** — use [`multi_channel::MultiChannel`] to manage indexed connections
//!   that share a single protection domain and memory region set.
//! * **Distributed coordination** — use [`network::Node`] for TCP-based endpoint exchange,
//!   rank/world-size management, and barrier synchronization across a cluster.
//! * **Low-level control** — the [`ibverbs`] module exposes the raw primitives (devices,
//!   protection domains, queue pairs, completion queues, memory regions, and work requests)
//!   for when you need full control over the RDMA stack.
//!
//! ## Quick example
//!
//! Open a device, build a channel, register memory, and send data in a scoped operation:
//!
//! ```no_run
//! use ibverbs_rs::ibverbs;
//! use ibverbs_rs::channel::Channel;
//! use ibverbs_rs::ibverbs::work::{SendWorkRequest, ReceiveWorkRequest};
//!
//! // Open device and allocate resources
//! let ctx = ibverbs::open_device("mlx5_0")?;
//! let pd = ctx.allocate_pd()?;
//! let prepared = Channel::builder().pd(&pd).build()?;
//!
//! // Exchange endpoints with the remote peer (here, loopback for illustration)
//! let endpoint = prepared.endpoint();
//! let mut channel = prepared.handshake(endpoint)?;
//!
//! // Register a buffer and perform a scoped send + receive
//! let mut buf = [0u8; 64];
//! let mr = pd.register_local_mr_slice(&buf)?;
//!
//! channel.scope(|s| {
//!     let (tx, rx) = buf.split_at_mut(32);
//!     s.post_send(SendWorkRequest::new(&[mr.gather_element(tx)]))?;
//!     s.post_receive(ReceiveWorkRequest::new(&mut [mr.scatter_element(rx)]))?;
//!     Ok::<(), ibverbs_rs::channel::ScopeError<ibverbs_rs::channel::TransportError>>(())
//! })?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Safety model
//!
//! Two-sided operations (send/receive) are lifetime-safe through Rust's borrow checker.
//! One-sided operations (RDMA read/write) require `unsafe` on the passive side, since the
//! remote peer can access registered memory at any time.
//!
//! See the [`ibverbs::memory`] module documentation for a detailed explanation of the
//! safety architecture.
//!
//! ## Cargo features
//!
//! * **`numa`** — Enables NUMA affinity helpers ([`ibverbs::numa`]) for pinning threads
//!   and memory allocations to the NUMA node local to an RDMA device. Requires `libnuma`
//!   to be installed on the system.

pub mod channel;
pub mod ibverbs;
pub mod multi_channel;
pub mod network;
