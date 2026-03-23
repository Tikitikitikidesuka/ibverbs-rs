/// A ranked network node with barrier synchronization.
///
/// Combines a [`MultiChannel`] with a rank, world size, and a [`Barrier`] for
/// collective synchronization across all nodes.
pub mod barrier;
pub mod builder;
pub mod config;
pub mod ops;
pub mod polling_scope;
pub mod tcp_exchanger;

use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::multi_channel::MultiChannel;
use crate::network::barrier::Barrier;
use crate::network::builder::NodeBuilder;
use crate::network::builder::node_builder::SetPd;

/// A ranked RDMA network node with barrier synchronization.
///
/// Wraps a [`MultiChannel`] with a rank, world size, and a [`Barrier`] for
/// collective synchronization across all nodes in the network.
#[derive(Debug)]
pub struct Node {
    rank: usize,
    world_size: usize,
    multi_channel: MultiChannel,
    barrier: Barrier,
}

impl Node {
    /// Returns the protection domain this node belongs to.
    pub fn pd(&self) -> &ProtectionDomain {
        self.multi_channel.pd()
    }

    /// Returns the total number of nodes in the network.
    pub fn world_size(&self) -> usize {
        self.world_size
    }

    /// Returns this node's rank (index) in the network.
    pub fn rank(&self) -> usize {
        self.rank
    }
}

impl ProtectionDomain {
    /// Creates a [`NodeBuilder`] under this protection domain.
    pub fn create_node(&self) -> NodeBuilder<'_, SetPd> {
        Node::builder().pd(self)
    }
}
