use crate::channel::builder::ChannelBuilder;
use crate::channel::builder::channel_builder::SetPd;
use crate::channel::cached_completion_queue::CachedCompletionQueue;
use crate::ibverbs::error::IbvError;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::ibverbs::queue_pair::QueuePair;
use std::cell::RefCell;
use std::rc::Rc;
use thiserror::Error;
use crate::ibverbs::work::WorkError;

pub mod builder;
pub mod pending_work;
pub mod polled_ops;
pub mod polling_scope;
pub mod remote_mr_exchanger;
pub mod scoped_ops;
pub mod unpolled_ops;

mod cached_completion_queue;

/// A rechannel is like the old connection but takes a shared protection domain.
/// This allows for creating a connection like the one that previously existed but
/// for optimizing the dessign of the network node.
/// If a network node is made with multiple connections, each with their own protection domain
/// the same memory has to ber registered to each one and be kept track of for operations.
/// By using channels, it is allowed to register only once to the shared protection domain and then
/// share the same MemoryRegion struct with all of them.
///
/// As of now, this is safe because the queue pair created does not allow for
/// remote writing of the memory. Otherwise, the memory aliasing rules would not be guaranteed.
#[derive(Debug)]
pub struct Channel {
    qp: QueuePair,
    cq: Rc<RefCell<CachedCompletionQueue>>,
    pd: ProtectionDomain,
    next_wr_id: u64,
}

impl Channel {
    pub fn pd(&self) -> &ProtectionDomain {
        &self.pd
    }
}

impl ProtectionDomain {
    pub fn create_channel(&self) -> ChannelBuilder<'_, SetPd> {
        Channel::builder().pd(self)
    }
}

#[derive(Debug, Error)]
pub enum TransportError {
    #[error(transparent)]
    IbvError(#[from] IbvError),
    #[error(transparent)]
    WorkError(#[from] WorkError),
}

pub type TransportResult<T> = Result<T, TransportError>;
