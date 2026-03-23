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

/// A network node is a MultiChannel with an id (rank) connected to all other nodes
/// of the network.
#[derive(Debug)]
pub struct Node {
    rank: usize,
    world_size: usize,
    multi_channel: MultiChannel,
    barrier: Barrier,
}

impl Node {
    pub fn pd(&self) -> &ProtectionDomain {
        self.multi_channel.pd()
    }

    pub fn world_size(&self) -> usize {
        self.world_size
    }

    pub fn rank(&self) -> usize {
        self.rank
    }
}

impl ProtectionDomain {
    pub fn create_node(&self) -> NodeBuilder<'_, SetPd> {
        Node::builder().pd(self)
    }
}
