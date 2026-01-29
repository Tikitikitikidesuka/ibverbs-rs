mod barrier;
pub mod builder;
pub mod config;
pub mod tcp_exchanger;

use crate::channel::multi_channel::MultiChannel;
use crate::network::barrier::CentralizedBarrier;
use std::borrow::Borrow;
use std::io;
use std::ops::{Deref, DerefMut};

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
    pub fn barrier_unchecked(&mut self, peers: &[usize]) -> io::Result<()> {
        println!("{:?}", self.barrier);
        self.barrier
            .barrier_unchecked(&mut self.multi_channel, peers)?;
        println!("{:?}", self.barrier);
        Ok(())
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

impl Node {
    pub fn rank(&self) -> usize {
        self.rank
    }
}
