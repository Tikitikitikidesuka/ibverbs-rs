//! Point-to-point RDMA channel — builder API with lifetime-safe operation posting and scope-based completion polling.
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
//! # Choosing `scope` vs `manual_scope`
//!
//! * Use [`scope`](Channel::scope) when you want automatic cleanup: any work not
//!   manually polled is polled to completion when the scope exits, even on panic.
//!   Errors are wrapped in [`ScopeError`] to distinguish closure errors from
//!   auto-poll errors.
//! * Use [`manual_scope`](Channel::manual_scope) when you want to poll everything
//!   yourself and get `Result<T, E>` directly. It panics if you leave work unpolled
//!   on the success path, acting as a safety net against forgotten completions.
//!
//! # Error handling
//!
//! Transport-layer errors are reported as [`TransportError`], which covers both
//! low-level ibverbs call failures and work completion errors.
//! [`Channel::scope`] wraps errors further in [`ScopeError`] to distinguish between
//! closure errors and errors discovered during automatic polling at scope exit.
//!
//! # Examples
//!
//! ## Blocking send and receive
//!
//! ```no_run
//! use ibverbs_rs::ibverbs;
//! use ibverbs_rs::channel::Channel;
//! use ibverbs_rs::ibverbs::work::{SendWorkRequest, ReceiveWorkRequest};
//!
//! let ctx = ibverbs::open_device("mlx5_0")?;
//! let pd = ctx.allocate_pd()?;
//! let prepared = Channel::builder().pd(&pd).build()?;
//!
//! // Exchange endpoints out-of-band (loopback for illustration)
//! let endpoint = prepared.endpoint();
//! let mut channel = prepared.handshake(endpoint)?;
//!
//! let mut buf = [0u8; 64];
//! let mr = pd.register_local_mr_slice(&buf)?;
//!
//! // Blocking receive (posts one WR and spins until complete)
//! channel.receive(ReceiveWorkRequest::new(&mut [mr.scatter_element(&mut buf)]))?;
//!
//! // Blocking send
//! channel.send(SendWorkRequest::new(&[mr.gather_element(&buf)]))?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Scoped operations
//!
//! ```no_run
//! use ibverbs_rs::ibverbs;
//! use ibverbs_rs::channel::{Channel, ScopeError, TransportError};
//! use ibverbs_rs::ibverbs::work::{SendWorkRequest, ReceiveWorkRequest};
//!
//! let ctx = ibverbs::open_device("mlx5_0")?;
//! let pd = ctx.allocate_pd()?;
//! let prepared = Channel::builder().pd(&pd).build()?;
//! let endpoint = prepared.endpoint();
//! let mut channel = prepared.handshake(endpoint)?;
//!
//! let mut buf = [0u8; 64];
//! let mr = pd.register_local_mr_slice(&buf)?;
//!
//! channel.scope(|s| {
//!     let (tx, rx) = buf.split_at_mut(32);
//!
//!     // Post both operations — they execute concurrently on the NIC
//!     let send = s.post_send(SendWorkRequest::new(&[mr.gather_element(tx)]))?;
//!     let recv = s.post_receive(ReceiveWorkRequest::new(&mut [mr.scatter_element(rx)]))?;
//!
//!     // Optionally poll individual handles for fine-grained control
//!     while send.poll().is_none() {}   // spin until complete
//!     while recv.poll().is_none() {}   // spin until complete
//!
//!     Ok::<(), ScopeError<TransportError>>(())
//! })?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! See also the [`examples/channel.rs`](https://github.com/Tikitikitikidesuka/ibverbs-rs/blob/main/examples/channel.rs) file
//! for a complete runnable example.
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
