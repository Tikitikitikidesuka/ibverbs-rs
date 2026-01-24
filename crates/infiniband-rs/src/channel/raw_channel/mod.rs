use crate::channel::raw_channel::cached_completion_queue::CachedCompletionQueue;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::ibverbs::queue_pair::QueuePair;
use std::cell::RefCell;
use std::rc::Rc;

pub mod builder;
pub mod pending_work;
pub mod polled_ops;
//pub mod scoped;
pub mod polling_scope;
pub mod unpolled_ops;

mod cached_completion_queue;
mod unsafe_member;
mod scoped;

/// A channel is like the old connection but takes a shared protection domain.
/// This allows for creating a connection like the one that previously existed but
/// for optimizing the dessign of the network node.
/// If a network node is made with multiple connections, each with their own protection domain
/// the same memory has to ber registered to each one and be kept track of for operations.
/// By using channels, it is allowed to register only once to the shared protection domain and then
/// share the same MemoryRegion struct with all of them.
///
/// As of now, this is safe because the queue pair created does not allow for
/// remote writing of the memory. Otherwise, the memory aliasing rules would not be guaranteed.
pub struct RawChannel {
    qp: QueuePair,
    cq: Rc<RefCell<CachedCompletionQueue>>,
    pd: ProtectionDomain,
    next_wr_id: u64,
}
