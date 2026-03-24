//! Safe, lifetime-checked RDMA operations.
//!
//! This module provides [`Channel`], a safe wrapper over a [`QueuePair`]
//! that uses Rust's borrow checker to prevent data races between the CPU and NIC.
//!
//! # Posting operations
//!
//! Operations can be posted at three levels of control:
//!
//! * **Blocking** — [`Channel::send`], [`Channel::receive`], [`Channel::write`], [`Channel::read`]
//!   post a single operation and block until it completes.
//! * **Scoped** — [`Channel::scope`] and [`Channel::manual_scope`] open a
//!   [`PollingScope`] where multiple operations can be posted and
//!   polled independently. The scope guarantees all work is completed before it returns.
//! * **Unpolled** — [`Channel::send_unpolled`], [`Channel::receive_unpolled`], etc. are `unsafe`
//!   and return raw [`PendingWork`] handles. These are the building
//!   blocks used by the higher-level APIs.

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
