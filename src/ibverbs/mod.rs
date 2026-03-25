//! Core RDMA primitives — device discovery, protection domains, queue pairs, memory registration, completion queues, and work requests.
//!
//! This module provides the low-level building blocks for RDMA programming. Higher-level
//! abstractions like [`Channel`](crate::channel::Channel) and [`Node`](crate::network::Node)
//! are built on top of these primitives.
//!
//! # Typical Workflow
//!
//! ```text
//! list_devices() / open_device()
//!     └─▶ Context
//!          ├─▶ create_cq()   →  CompletionQueue ──────────────────┐
//!          └─▶ allocate_pd() →  ProtectionDomain                  │
//!               ├─▶ register_local_mr()  →  MemoryRegion          │
//!               └─▶ create_qp()                                   │
//!                    └─▶ .send_cq(&cq).recv_cq(&cq) ◀─────────────┘
//!                         └─▶ PreparedQueuePair
//!                                  └─▶ handshake()  →  QueuePair
//! ```
//!
//! For most use cases, prefer the [`Channel`](crate::channel::Channel) abstraction which wraps
//! this workflow and adds lifetime-safe operation posting. Use the primitives directly when
//! you need fine-grained control over resource lifetimes, custom completion queue topologies,
//! or integration with an existing event loop that the higher-level API cannot accommodate.

pub mod access_config;
pub mod completion_queue;
pub mod device;
pub mod error;
pub mod memory;
pub mod protection_domain;
pub mod queue_pair;
pub mod work;

#[cfg(feature = "numa")]
pub mod numa;

pub use device::list_devices;
pub use device::open_device;
