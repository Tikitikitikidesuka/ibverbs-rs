use crate::channel::builder::ChannelBuilder;
use crate::channel::builder::channel_builder::SetPd;
use crate::channel::cached_completion_queue::CachedCompletionQueue;
use crate::ibverbs::error::IbvError;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::ibverbs::queue_pair::QueuePair;
use crate::ibverbs::work::WorkError;
use std::cell::RefCell;
use std::rc::Rc;
use thiserror::Error;

pub mod builder;
pub mod pending_work;
pub mod polling_scope;

mod cached_completion_queue;
mod ops;

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
    /// Returns a [`ChannelBuilder`] with this protection domain already set.
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
