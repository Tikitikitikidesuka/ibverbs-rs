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
///
/// Barriers are **poisoned** after any error: once a `Barrier` returns an `Err`,
/// every subsequent call will immediately return [`Poisoned`](BarrierError::Poisoned)
/// without attempting any RDMA operations. This prevents use of a barrier whose
/// internal epoch state may be inconsistent with remote peers.
#[derive(Debug, Error)]
pub enum BarrierError {
    /// The barrier was poisoned by a previous error and can no longer be used.
    #[error("Barrier is poisoned from a previous error")]
    Poisoned,
    /// This node's own rank is not present in the supplied peer list.
    #[error("Self not in the barrier group")]
    SelfNotInGroup,
    /// The peer list is not sorted in strictly ascending order.
    #[error("Peers not in ascending order in barrier group")]
    UnorderedPeers,
    /// The peer list contains the same rank more than once.
    #[error("Duplicate peers in barrier group")]
    DuplicatePeers,
    /// Not all peers reached the barrier within the allotted time.
    #[error("Barrier timeout")]
    Timeout,
    /// An RDMA transport error occurred while exchanging barrier notifications.
    #[error("Transport error: {0}")]
    TransportError(#[from] TransportError),
}

/// Selects which barrier algorithm a [`Node`](crate::network::Node) uses.
///
/// All algorithms are implemented over one-sided RDMA writes and spin-poll on
/// a local memory region, so no CPU involvement is required on the remote side
/// during the synchronization itself.
///
/// The algorithm is chosen once at node construction and cannot be changed
/// afterwards. The default used by [`Node::builder`](crate::network::Node::builder)
/// is [`BinaryTree`](BarrierAlgorithm::BinaryTree).
#[derive(Debug, Copy, Clone)]
pub enum BarrierAlgorithm {
    /// Leader-based barrier. The lowest-ranked participant collects a notification
    /// from every other peer, then broadcasts back. O(n) messages; simple and
    /// correct but does not scale with large groups.
    Centralized,
    /// Tree-structured barrier. Peers are arranged in a binary tree by their
    /// position in the sorted peer list. A reduce phase propagates notifications
    /// from leaves up to the root, followed by a broadcast phase back down.
    /// O(log n) rounds; balanced and generally a good default.
    BinaryTree,
    /// Dissemination barrier. In each round every peer notifies the peer at
    /// distance `d` to its right (circularly) and waits for the peer at
    /// distance `d` to its left. The distance doubles each round: 1, 2, 4, …
    /// Completes in ⌈log₂ n⌉ rounds with no designated leader and no single
    /// point of contention.
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

/// A connected barrier ready to synchronize with peers.
///
/// Normally accessed through [`Node::barrier`](crate::network::Node::barrier) rather
/// than directly.
///
/// # Peer list contract
///
/// Both [`barrier`](Barrier::barrier) and [`barrier_unchecked`](Barrier::barrier_unchecked)
/// require the peer list to obey the following rules (only [`barrier`](Barrier::barrier)
/// validates them):
///
/// * **Sorted** — ranks must appear in strictly ascending order.
/// * **No duplicates** — each rank may appear at most once.
/// * **Self included** — this node's own rank must be present.
///
/// # Poisoning
///
/// If any call returns an error, the barrier is permanently poisoned and all
/// subsequent calls return [`BarrierError::Poisoned`] immediately. Recreate the
/// [`Node`](crate::network::Node) to recover.
#[derive(Debug)]
pub enum Barrier {
    /// Leader-based barrier. See [`BarrierAlgorithm::Centralized`].
    Centralized(LinearBarrier),
    /// Binary-tree barrier. See [`BarrierAlgorithm::BinaryTree`].
    BinaryTree(BinaryTreeBarrier),
    /// Dissemination barrier. See [`BarrierAlgorithm::Dissemination`].
    Dissemination(DisseminationBarrier),
}

impl Barrier {
    fn new_centralized(
        pd: &ProtectionDomain,
        rank: usize,
        world_size: usize,
    ) -> IbvResult<PreparedBarrier> {
        Ok(PreparedBarrier::Centralized(PreparedLinearBarrier::new(
            pd, rank, world_size,
        )?))
    }

    fn new_binary_tree(
        pd: &ProtectionDomain,
        rank: usize,
        world_size: usize,
    ) -> IbvResult<PreparedBarrier> {
        Ok(PreparedBarrier::BinaryTree(PreparedBinaryTreeBarrier::new(
            pd, rank, world_size,
        )?))
    }

    fn new_dissemination(
        pd: &ProtectionDomain,
        rank: usize,
        world_size: usize,
    ) -> IbvResult<PreparedBarrier> {
        Ok(PreparedBarrier::Dissemination(
            PreparedDisseminationBarrier::new(pd, rank, world_size)?,
        ))
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
    /// Leader-based barrier. See [`BarrierAlgorithm::Centralized`].
    Centralized(PreparedLinearBarrier),
    /// Binary-tree barrier. See [`BarrierAlgorithm::BinaryTree`].
    BinaryTree(PreparedBinaryTreeBarrier),
    /// Dissemination barrier. See [`BarrierAlgorithm::Dissemination`].
    Dissemination(PreparedDisseminationBarrier),
}

/// Validates that a peer list is sorted and contains no duplicates.
///
/// Used by barrier implementations before dispatching to the algorithm-specific logic.
pub(super) fn validate_peer_list(peers: &[usize]) -> Result<(), BarrierError> {
    if !peers.is_sorted() {
        return Err(BarrierError::UnorderedPeers);
    }
    if peers.windows(2).any(|w| w[0] == w[1]) {
        return Err(BarrierError::DuplicatePeers);
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_sorted_unique_peers() {
        assert!(validate_peer_list(&[0, 1, 2, 3]).is_ok());
    }

    #[test]
    fn empty_peer_list_is_valid() {
        assert!(validate_peer_list(&[]).is_ok());
    }

    #[test]
    fn single_peer_is_valid() {
        assert!(validate_peer_list(&[5]).is_ok());
    }

    #[test]
    fn non_contiguous_ranks_are_valid() {
        assert!(validate_peer_list(&[0, 3, 7, 100]).is_ok());
    }

    #[test]
    fn unsorted_peers_rejected() {
        let err = validate_peer_list(&[2, 1, 3]).unwrap_err();
        assert!(matches!(err, BarrierError::UnorderedPeers));
    }

    #[test]
    fn duplicate_peers_rejected() {
        let err = validate_peer_list(&[0, 1, 1, 2]).unwrap_err();
        assert!(matches!(err, BarrierError::DuplicatePeers));
    }

    #[test]
    fn unsorted_takes_precedence_over_duplicate() {
        // [2, 1, 1] — unsorted check triggers first
        let err = validate_peer_list(&[2, 1, 1]).unwrap_err();
        assert!(matches!(err, BarrierError::UnorderedPeers));
    }

    #[test]
    fn all_same_ranks_detected_as_unsorted_or_duplicate() {
        // [3, 3, 3] — is_sorted() returns true for equal elements, so duplicates detected
        let err = validate_peer_list(&[3, 3, 3]).unwrap_err();
        assert!(matches!(err, BarrierError::DuplicatePeers));
    }
}
