pub mod builder;
pub mod config;
pub mod tcp_exchanger;

use crate::channel::multi_channel::MultiChannel;
use std::ops::{Deref, DerefMut};

/// A network node is a MultiChannel with an id (rank) connected to all other nodes
/// of the network.
pub struct Node {
    rank: usize,
    num_network_nodes: usize,
    multi_channel: MultiChannel,
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

impl Node {}
