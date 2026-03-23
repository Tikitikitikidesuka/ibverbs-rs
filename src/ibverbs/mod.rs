//! Core RDMA primitives.
//!
//! This module provides the low-level building blocks for RDMA programming. Higher-level
//! abstractions like [`Channel`](crate::channel::Channel) and [`Node`](crate::network::Node)
//! are built on top of these primitives.
//!
//! # Submodules
//!
//! * [`device`] — Device discovery, context management, and GUID handling.
//! * [`protection_domain`] — Resource isolation — groups MRs, QPs, and other objects that can
//!   interact with each other.
//! * [`memory`] — Memory registration, scatter/gather elements, and remote memory handles.
//! * [`queue_pair`] — The communication endpoint for posting RDMA operations.
//! * [`completion_queue`] — Polling for completed work requests.
//! * [`work`] — Work request types (Send, Receive, Write, Read) and completion results.
//! * [`access_config`] — Access permission flags for memory regions and queue pairs.
//! * [`error`] — Error types for ibverbs operations.
//!
//! # Typical Workflow
//!
//! ```text
//! list_devices() / open_device()
//!     └─▶ Context
//!          ├─▶ allocate_pd()  →  ProtectionDomain
//!          │    ├─▶ register_local_mr()  →  MemoryRegion
//!          │    └─▶ create_qp()  →  QueuePairBuilder  →  PreparedQueuePair
//!          │                                                  └─▶ handshake()  →  QueuePair
//!          └─▶ create_cq()  →  CompletionQueue
//! ```
//!
//! For most use cases, prefer the [`Channel`](crate::channel::Channel) abstraction which wraps
//! this workflow and adds lifetime-safe operation posting.

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
