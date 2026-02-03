use crate::channel::TransportError;
use crate::ibverbs::error::IbvResult;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::multi_channel::MultiChannel;
use crate::multi_channel::remote_memory_region::PeerRemoteMemoryRegion;
use crate::network::barrier::binary_tree::{BinaryTreeBarrier, PreparedBinaryTreeBarrier};
use crate::network::barrier::centralized::{CentralizedBarrier, PreparedCentralizedBarrier};
use std::time::Duration;
use thiserror::Error;

pub mod binary_tree;
pub mod centralized;
mod memory;

#[derive(Debug, Error)]
pub enum BarrierError {
    #[error("Barrier is poisoned from a previous error")]
    Poisoned,
    #[error("Self not in the barrier group")]
    SelfNotInGroup,
    #[error("Peers not in ascending order in barrier group")]
    UnorderedPeers,
    #[error("Duplicate peers in barrier group")]
    DuplicatePeers,
    #[error("Barrier timeout")]
    Timeout,
    #[error("Transport error: {0}")]
    TransportError(#[from] TransportError),
}

#[derive(Debug, Copy, Clone)]
pub enum BarrierAlgorithm {
    Centralized,
    BinaryTree,
}

impl BarrierAlgorithm {
    pub fn instance(
        &self,
        pd: &ProtectionDomain,
        rank: usize,
        world_size: usize,
    ) -> IbvResult<PreparedBarrier> {
        match self {
            BarrierAlgorithm::Centralized => Barrier::new_centralized(pd, rank, world_size),
            BarrierAlgorithm::BinaryTree => Barrier::new_binary_tree(pd, rank, world_size),
        }
    }
}

#[derive(Debug)]
pub enum Barrier {
    Centralized(CentralizedBarrier),
    BinaryTree(BinaryTreeBarrier),
}

impl Barrier {
    pub fn new_centralized(
        pd: &ProtectionDomain,
        rank: usize,
        world_size: usize,
    ) -> IbvResult<PreparedBarrier> {
        Ok(PreparedBarrier::Centralized(CentralizedBarrier::new(
            pd, rank, world_size,
        )?))
    }

    pub fn new_binary_tree(
        pd: &ProtectionDomain,
        rank: usize,
        world_size: usize,
    ) -> IbvResult<PreparedBarrier> {
        Ok(PreparedBarrier::BinaryTree(BinaryTreeBarrier::new(
            pd, rank, world_size,
        )?))
    }

    pub fn barrier(
        &mut self,
        multi_channel: &mut MultiChannel,
        peers: &[usize],
        timeout: Duration,
    ) -> Result<(), BarrierError> {
        match self {
            Barrier::Centralized(b) => b.barrier(multi_channel, peers, timeout),
            Barrier::BinaryTree(b) => b.barrier(multi_channel, peers, timeout),
        }
    }

    /// Assumes peers are ordered, non repeating and self is in the group
    pub fn barrier_unchecked(
        &mut self,
        multi_channel: &mut MultiChannel,
        peers: &[usize],
        timeout: Duration,
    ) -> Result<(), BarrierError> {
        match self {
            Barrier::Centralized(b) => b.barrier_unchecked(multi_channel, peers, timeout),
            Barrier::BinaryTree(b) => b.barrier_unchecked(multi_channel, peers, timeout),
        }
    }
}

#[derive(Debug)]
pub enum PreparedBarrier {
    Centralized(PreparedCentralizedBarrier),
    BinaryTree(PreparedBinaryTreeBarrier),
}

impl PreparedBarrier {
    pub fn remote_mr(&self) -> PeerRemoteMemoryRegion {
        match self {
            PreparedBarrier::Centralized(p) => p.remote(),
            PreparedBarrier::BinaryTree(p) => p.remote(),
        }
    }

    pub fn link_remote(self, remote_mrs: Box<[PeerRemoteMemoryRegion]>) -> Barrier {
        match self {
            PreparedBarrier::Centralized(p) => Barrier::Centralized(p.link_remote(remote_mrs)),
            PreparedBarrier::BinaryTree(p) => Barrier::BinaryTree(p.link_remote(remote_mrs)),
        }
    }
}
