//! Queue pair — the communication endpoint for RDMA operations.
//!
//! # The Communication Endpoint
//!
//! A [`QueuePair`] is the fundamental object used to send and receive data in RDMA.
//!
//! # Key Relationships
//!
//! * **Protection Domain (PD)** — A QP is created within a specific [`ProtectionDomain`].
//!   **Crucial Rule**: You can only use [`MemoryRegion`](crate::ibverbs::memory::MemoryRegion)s
//!   that were registered in the *same* PD. Mixing PDs will cause immediate errors.
//!
//! * **Completion Queues (CQ)** — A QP is associated with two Completion Queues (which can be the same object):
//!   * **Send CQ** — Receives completions for outgoing operations (Send, Write, Read).
//!   * **Recv CQ** — Receives completions for incoming operations (Receive).
//!   * *Note*: When an operation finishes, the hardware places a [`WorkCompletion`](crate::ibverbs::work::WorkCompletion)
//!     into the corresponding CQ. You must poll that CQ to see the result.
//!
//! # Usage: The Post-and-Poll Model
//!
//! Using a Queue Pair follows a strict asynchronous pattern:
//!
//! 1.  **Post**: You submit a work request using methods like [`post_send`](QueuePair::post_send).
//!     This method returns immediately (non-blocking).
//! 2.  **Execute**: The hardware processes the request asynchronously in the background.
//! 3.  **Complete**: Eventually, a completion event appears in the associated CQ. You poll the CQ
//!     using the `wr_id` you assigned to retrieve the result.
//!
//! # Safety Model
//!
//! The `post_*` methods are **`unsafe`** because they do not enforce buffer lifetime safety automatically.
//!
//! ## The Contract
//!
//! When you create a [`WorkRequest`](crate::ibverbs::work), the [`GatherElement`](crate::ibverbs::memory::GatherElement)
//! and [`ScatterElement`](crate::ibverbs::memory::ScatterElement) types capture the lifetime of your data buffers
//! through Rust's borrow checker. However, **the `QueuePair` does not return a handle** that ties this
//! lifetime to the completion of the operation.
//!
//! **It is your responsibility to**:
//! 1.  Keep the data buffers alive until the operation completes.
//! 2.  Not mutate (for Send/Write) or access (for Receive/Read) the buffers while the hardware owns them.
//! 3.  Poll the appropriate Completion Queue using the `wr_id` to know when the operation finishes.
//!
//! Only after you receive the [`WorkCompletion`](crate::ibverbs::work::WorkCompletion) is it safe to
//! alias, drop, reuse, or modify the buffers.
//!
//! ## Safe Abstractions
//!
//! A higher-level abstraction [`Channel`](crate::channel::Channel) wraps these `unsafe` methods and enforces
//! the lifetime contract by returning a future or handle that must be awaited/polled before the
//! buffers are released.

pub mod builder;
pub mod config;
pub mod ops;

use crate::ibverbs::completion_queue::CompletionQueue;
use crate::ibverbs::error::IbvError;
use crate::ibverbs::protection_domain::ProtectionDomain;
use ibverbs_sys::{ibv_destroy_qp, ibv_qp};
use std::fmt::Debug;

/// An RDMA Queue Pair.
///
/// This struct manages the lifecycle of the QP resource. It holds strong references to its
/// dependencies ([`ProtectionDomain`] and [`CompletionQueue`]) to ensure they remain
/// allocated as long as this QP exists.
pub struct QueuePair {
    pd: ProtectionDomain,
    // Kept to ensure CQs are not dropped before the QP
    _send_cq: CompletionQueue,
    _recv_cq: CompletionQueue,
    qp: *mut ibv_qp,
}

/// SAFETY: libibverbs resources are thread-safe.
unsafe impl Send for QueuePair {}
/// SAFETY: libibverbs resources are thread-safe.
unsafe impl Sync for QueuePair {}

impl Drop for QueuePair {
    fn drop(&mut self) {
        log::debug!("QueuePair destroyed");
        let errno = unsafe { ibv_destroy_qp(self.qp) };
        if errno != 0 {
            let error = IbvError::from_errno_with_msg(errno, "Failed to destroy queue pair");
            log::error!("{error}");
        }
    }
}

impl Debug for QueuePair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.debug_struct("QueuePair")
            .field("handle", &unsafe { (*self.qp).handle })
            .field("qp_num", &unsafe { (*self.qp).qp_num })
            .field("state", &unsafe { (*self.qp).state })
            .field("type", &unsafe { (*self.qp).qp_type })
            .field("send_cq_handle", &unsafe { (*(*self.qp).send_cq).handle })
            .field("recv_cq_handle", &unsafe { (*(*self.qp).recv_cq).handle })
            .field("pd", &self.pd)
            .finish()
    }
}
