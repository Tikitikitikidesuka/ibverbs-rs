//! A safe, lifetime-checked RDMA channel between two peers.
//!
//! A [`Channel`] wraps an InfiniBand [`QueuePair`] and uses Rust's borrow checker
//! to statically prevent data races between the CPU and the NIC: memory passed to a
//! work request is borrowed for the duration of the operation, so the compiler rejects
//! any attempt to read or drop it while the hardware may still be performing DMA.
//!
//! # Connection lifecycle
//!
//! A channel is established in two steps so that the endpoints can exchange connection
//! information out-of-band (typically over TCP) before the RDMA link is brought up.
//!
//! 1. **Build** — call [`Channel::builder`] (or [`ProtectionDomain::create_channel`])
//!    and configure the queue pair parameters. [`build`](ChannelBuilder::build) returns
//!    a [`PreparedChannel`] whose [`endpoint`](PreparedChannel::endpoint) contains the
//!    local connection information.
//! 2. **Handshake** — exchange [`QueuePairEndpoint`]s with the remote peer, then call
//!    [`PreparedChannel::handshake`] to bring the queue pair to the Ready-To-Send state
//!    and obtain the connected [`Channel`].
//!
//! # Posting operations
//!
//! Once connected, operations can be posted at three levels of control:
//!
//! * **Blocking** — [`Channel::send`], [`Channel::receive`], [`Channel::write`],
//!   [`Channel::read`] each post a single operation and spin-poll until it completes.
//!   Best for simple, sequential use cases.
//! * **Scoped** — [`Channel::scope`] and [`Channel::manual_scope`] open a
//!   [`PollingScope`] through which multiple operations can be posted and polled
//!   independently as [`ScopedPendingWork`] handles. The scope guarantees all
//!   outstanding work is polled to completion before it returns, even if the closure
//!   panics. This mirrors the design of [`std::thread::scope`].
//! * **Unpolled** — [`Channel::send_unpolled`], [`Channel::receive_unpolled`],
//!   [`Channel::write_unpolled`], [`Channel::read_unpolled`] are `unsafe` and return
//!   raw [`PendingWork`] handles. These are the primitives that the two higher levels
//!   are built on; prefer those unless you need direct control.
//!
//! # Memory safety
//!
//! Work requests borrow their data buffers for the lifetime of the operation. That
//! borrow is released only once the operation is polled to completion — or, if the
//! handle is dropped without being polled, by blocking until the hardware finishes.
//! It is therefore impossible in safe code to free or reuse a buffer that the NIC is
//! still reading from or writing to.
//!
//! # Error handling
//!
//! Transport-layer errors are reported as [`TransportError`], which covers both
//! low-level ibverbs call failures and work completion errors.
//! [`Channel::scope`] wraps errors further in [`ScopeError`] to distinguish between
//! closure errors and errors discovered during automatic polling at scope exit.
//!
//! [`QueuePair`]: crate::ibverbs::queue_pair::QueuePair
//! [`QueuePairEndpoint`]: crate::ibverbs::queue_pair::builder::QueuePairEndpoint

use crate::channel::cached_completion_queue::CachedCompletionQueue;
use crate::ibverbs::error::IbvError;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::ibverbs::queue_pair::QueuePair;
use crate::ibverbs::work::WorkError;
use std::cell::RefCell;
use std::rc::Rc;
use thiserror::Error;

mod builder;
mod cached_completion_queue;
mod ops;
mod pending_work;
mod polling_scope;

#[doc(hidden)]
pub use builder::channel_builder::{
    Empty, SetAccess, SetAckTimeout, SetMaxAckRetries, SetMaxRecvSge, SetMaxRecvWr,
    SetMaxRnrRetries, SetMaxSendSge, SetMaxSendWr, SetMinCqEntries, SetMinRnrTimer, SetMtu, SetPd,
    SetRecvPsn, SetSendPsn,
};
pub use builder::{ChannelBuilder, PreparedChannel};
pub use pending_work::PendingWork;
pub use polling_scope::{PollingScope, ScopeError, ScopeResult, ScopedPendingWork};

/// A safe RDMA communication endpoint built on top of a [`QueuePair`].
///
/// `Channel` wraps a queue pair with lifetime-safe operation posting through
/// [`scope`](Self::scope) and [`manual_scope`](Self::manual_scope).
///
/// A channel belongs to a [`ProtectionDomain`] and can share memory regions with
/// other channels under the same domain.
/// Use [`ProtectionDomain::create_channel`] or [`Channel::builder`] to construct one.
#[derive(Debug)]
pub struct Channel {
    qp: QueuePair,
    cq: Rc<RefCell<CachedCompletionQueue>>,
    pd: ProtectionDomain,
    next_wr_id: u64,
}

impl Channel {
    /// Returns a reference to the channel's [`ProtectionDomain`].
    pub fn pd(&self) -> &ProtectionDomain {
        &self.pd
    }
}

impl ProtectionDomain {
    /// Returns a builder with this protection domain already set.
    pub fn create_channel(&self) -> ChannelBuilder<'_, SetPd> {
        Channel::builder().pd(self)
    }
}

/// An error from an RDMA transport operation.
///
/// Wraps both low-level ibverbs errors and work completion errors into a single type.
#[derive(Debug, Error)]
pub enum TransportError {
    #[error(transparent)]
    IbvError(#[from] IbvError),
    #[error(transparent)]
    WorkError(#[from] WorkError),
}

/// Convenience alias for a [`Result`] with [`TransportError`].
pub type TransportResult<T> = Result<T, TransportError>;
