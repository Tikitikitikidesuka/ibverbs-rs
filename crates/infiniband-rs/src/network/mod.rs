pub mod barrier;
pub mod builder;
pub mod config;
pub mod tcp_exchanger;

use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::multi_channel::MultiChannel;
use crate::network::barrier::{BarrierError, CentralizedBarrier};
use crate::network::builder::NodeBuilder;
use crate::network::builder::node_builder::SetPd;
use std::ops::{Deref, DerefMut};
use std::time::Duration;

/// A network node is a MultiChannel with an id (rank) connected to all other nodes
/// of the network.
#[derive(Debug)]
pub struct Node {
    rank: usize,
    world_size: usize,
    multi_channel: MultiChannel,
    barrier: CentralizedBarrier,
}

impl Node {
    pub fn rank(&self) -> usize {
        self.rank
    }

    pub fn world_size(&self) -> usize {
        self.world_size
    }

    pub fn barrier(&mut self, peers: &[usize], timeout: Duration) -> Result<(), BarrierError> {
        self.barrier
            .barrier(&mut self.multi_channel, peers, timeout)
    }

    pub fn barrier_unchecked(
        &mut self,
        peers: &[usize],
        timeout: Duration,
    ) -> Result<(), BarrierError> {
        self.barrier
            .barrier_unchecked(&mut self.multi_channel, peers, timeout)
    }
}

impl ProtectionDomain {
    pub fn create_node(&self) -> NodeBuilder<'_, SetPd> {
        Node::builder().pd(self)
    }
}

impl Deref for Node {
    type Target = MultiChannel;

    fn deref(&self) -> &Self::Target {
        &self.multi_channel
    }
}

impl DerefMut for Node {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.multi_channel
    }
}
