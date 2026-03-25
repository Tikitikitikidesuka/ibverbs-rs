//! Access flags — permission bits for memory regions and queue pairs.
//!
//! This module defines the [`AccessFlags`] struct, which controls the allowed operations
//! on RDMA resources.
//!
//! # Usage Contexts
//!
//! These flags are used in two primary contexts:
//!
//! ## 1. Memory Region Registration
//!
//! When registering memory (e.g., [`MemoryRegion::register_mr_with_access`](crate::ibverbs::memory::MemoryRegion::register_mr_with_access)),
//! these flags define what operations the hardware is allowed to perform on that specific buffer.
//!
//! * **`LOCAL_WRITE`** — Required if you want to use the MR as a destination for Receive or RDMA Read.
//! * **`REMOTE_WRITE`** — Allows remote peers to write into this MR.
//! * **`REMOTE_READ`** — Allows remote peers to read from this MR.
//!
//! ## 2. Queue Pair Configuration
//!
//! When creating a Queue Pair (e.g., [`QueuePair::builder`](crate::ibverbs::queue_pair::QueuePair::builder)),
//! these flags define the capabilities of the *incoming* channel. They act as a "gatekeeper" for the entire connection.
//!
//! * If a QP is created without `REMOTE_WRITE`, all incoming RDMA Write requests will be rejected,
//!   even if the target Memory Region has `REMOTE_WRITE` enabled.
//!
//! # Example
//!
//! ```
//! use ibverbs_rs::ibverbs::access_config::AccessFlags;
//!
//! // Local write only (the default for safe MR registration)
//! let local = AccessFlags::new().with_local_write();
//!
//! // Full remote access (required for one-sided RDMA targets)
//! let shared = AccessFlags::new()
//!     .with_local_write()
//!     .with_remote_read()
//!     .with_remote_write();
//! ```
//!
//! # Safety
//!
//! Enabling remote access flags (`REMOTE_WRITE`, `REMOTE_READ`) on a Memory Region introduces
//! safety concerns (aliasing and lifetime management). See the [`memory`](crate::ibverbs::memory)
//! module documentation for details.

use ibverbs_sys::ibv_access_flags;

/// A bitmask of allowed RDMA operations.
#[derive(Debug, Copy, Clone)]
pub struct AccessFlags(u32);

impl AccessFlags {
    /// Creates new access flags with no flags set.
    pub fn new() -> Self {
        Self(0)
    }

    /// Enables **Local Write** access.
    ///
    /// * **For Memory Regions** — Allows the local NIC to write to this memory (e.g., during Receive or RDMA Read).
    ///   This is required for any buffer that will hold incoming data.
    /// * **For Queue Pairs** — Ignored/Implied (Local write permission is property of the MR).
    pub fn with_local_write(mut self) -> Self {
        self.0 |= ibv_access_flags::IBV_ACCESS_LOCAL_WRITE.0;
        self
    }

    /// Enables **Remote Read** access.
    ///
    /// * **For Memory Regions** — Allows remote peers to read data from this region via RDMA Read.
    /// * **For Queue Pairs** — Allows the QP to process incoming RDMA Read requests.
    pub fn with_remote_read(mut self) -> Self {
        self.0 |= ibv_access_flags::IBV_ACCESS_REMOTE_READ.0;
        self
    }

    /// Enables **Remote Write** access.
    ///
    /// * **For Memory Regions** — Allows remote peers to write data to this region via RDMA Write.
    ///   **Note**: This implicitly enables `LOCAL_WRITE` in most hardware implementations.
    /// * **For Queue Pairs** — Allows the QP to process incoming RDMA Write requests.
    pub fn with_remote_write(mut self) -> Self {
        self.0 |= ibv_access_flags::IBV_ACCESS_REMOTE_WRITE.0;
        self
    }

    /// Returns the raw `ibverbs` bitmask.
    pub fn code(&self) -> u32 {
        self.0
    }
}

impl Default for AccessFlags {
    /// Defaults to `LOCAL_WRITE` enabled.
    ///
    /// This is the most common configuration, allowing local send/receive operations
    /// but disallowing one-sided remote access.
    fn default() -> AccessFlags {
        AccessFlags::new().with_local_write()
    }
}
