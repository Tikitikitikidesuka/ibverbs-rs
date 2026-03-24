use crate::channel::TransportError;
use crate::ibverbs::error::IbvResult;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::multi_channel::MultiChannel;
use crate::multi_channel::PeerRemoteMemoryRegion;
use crate::network::barrier::binary_tree::{BinaryTreeBarrier, PreparedBinaryTreeBarrier};
use crate::network::barrier::dissemination::{DisseminationBarrier, PreparedDisseminationBarrier};
use crate::network::barrier::linear::{LinearBarrier, PreparedLinearBarrier};
use std::time::Duration;
use thiserror::Error;

mod binary_tree;
mod dissemination;
mod linear;
mod memory;

/// An error that can occur during a barrier synchronization.
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

/// Selects which barrier algorithm to use.
///
/// * `Centralized` — Leader-based linear barrier. Simple but O(n).
/// * `BinaryTree` — Tree reduction and broadcast. O(log n).
/// * `Dissemination` — Pairwise exchange at exponential distances. O(log n), no designated leader.
#[derive(Debug, Copy, Clone)]
pub enum BarrierAlgorithm {
    Centralized,
    BinaryTree,
    Dissemination,
}

impl BarrierAlgorithm {
    /// Creates a [`PreparedBarrier`] using this algorithm.
    pub fn instance(
        &self,
        pd: &ProtectionDomain,
        rank: usize,
        world_size: usize,
    ) -> IbvResult<PreparedBarrier> {
        match self {
            BarrierAlgorithm::Centralized => Barrier::new_centralized(pd, rank, world_size),
            BarrierAlgorithm::BinaryTree => Barrier::new_binary_tree(pd, rank, world_size),
            BarrierAlgorithm::Dissemination => Barrier::new_dissemination(pd, rank, world_size),
        }
    }
}

/// A connected barrier, ready to synchronize with peers.
///
/// Dispatches to the concrete algorithm selected at construction time.
#[derive(Debug)]
pub enum Barrier {
    Centralized(LinearBarrier),
    BinaryTree(BinaryTreeBarrier),
    Dissemination(DisseminationBarrier),
}

impl Barrier {
    fn new_centralized(
        pd: &ProtectionDomain,
        rank: usize,
        world_size: usize,
    ) -> IbvResult<PreparedBarrier> {
        Ok(PreparedBarrier::Centralized(LinearBarrier::new(
            pd, rank, world_size,
        )?))
    }

    fn new_binary_tree(
        pd: &ProtectionDomain,
        rank: usize,
        world_size: usize,
    ) -> IbvResult<PreparedBarrier> {
        Ok(PreparedBarrier::BinaryTree(BinaryTreeBarrier::new(
            pd, rank, world_size,
        )?))
    }

    fn new_dissemination(
        pd: &ProtectionDomain,
        rank: usize,
        world_size: usize,
    ) -> IbvResult<PreparedBarrier> {
        Ok(PreparedBarrier::Dissemination(DisseminationBarrier::new(
            pd, rank, world_size,
        )?))
    }

    /// Synchronizes with the given peers, blocking until all have reached the barrier or timeout.
    pub fn barrier(
        &mut self,
        multi_channel: &mut MultiChannel,
        peers: &[usize],
        timeout: Duration,
    ) -> Result<(), BarrierError> {
        match self {
            Barrier::Centralized(b) => b.barrier(multi_channel, peers, timeout),
            Barrier::BinaryTree(b) => b.barrier(multi_channel, peers, timeout),
            Barrier::Dissemination(b) => b.barrier(multi_channel, peers, timeout),
        }
    }

    /// Like [`barrier`](Self::barrier), but skips validation of the peer list.
    pub fn barrier_unchecked(
        &mut self,
        multi_channel: &mut MultiChannel,
        peers: &[usize],
        timeout: Duration,
    ) -> Result<(), BarrierError> {
        match self {
            Barrier::Centralized(b) => b.barrier_unchecked(multi_channel, peers, timeout),
            Barrier::BinaryTree(b) => b.barrier_unchecked(multi_channel, peers, timeout),
            Barrier::Dissemination(b) => b.barrier_unchecked(multi_channel, peers, timeout),
        }
    }
}

/// A barrier that has been allocated but not yet linked to remote peers.
///
/// Call [`link_remote`](Self::link_remote) with the remote memory region handles
/// after the endpoint exchange to produce a [`Barrier`].
#[derive(Debug)]
pub enum PreparedBarrier {
    Centralized(PreparedLinearBarrier),
    BinaryTree(PreparedBinaryTreeBarrier),
    Dissemination(PreparedDisseminationBarrier),
}

impl PreparedBarrier {
    /// Returns this node's barrier memory region handle for exchange with peers.
    pub fn remote_mr(&self) -> PeerRemoteMemoryRegion {
        match self {
            PreparedBarrier::Centralized(p) => p.remote(),
            PreparedBarrier::BinaryTree(p) => p.remote(),
            PreparedBarrier::Dissemination(p) => p.remote(),
        }
    }

    /// Links remote peer memory regions and returns a ready-to-use [`Barrier`].
    pub fn link_remote(self, remote_mrs: Box<[PeerRemoteMemoryRegion]>) -> Barrier {
        match self {
            PreparedBarrier::Centralized(p) => Barrier::Centralized(p.link_remote(remote_mrs)),
            PreparedBarrier::BinaryTree(p) => Barrier::BinaryTree(p.link_remote(remote_mrs)),
            PreparedBarrier::Dissemination(p) => Barrier::Dissemination(p.link_remote(remote_mrs)),
        }
    }
}
