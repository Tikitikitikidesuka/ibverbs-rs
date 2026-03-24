use crate::ibverbs::error::IbvResult;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::multi_channel::MultiChannel;
use crate::multi_channel::PeerRemoteMemoryRegion;
use crate::network::barrier::BarrierError;
use crate::network::barrier::memory::{BarrierMr, PreparedBarrierMr};
use std::time::{Duration, Instant};

/// Binary tree barrier implementation.
///
/// Nodes are arranged in a binary tree. The reduce phase propagates notifications
/// upward from leaves to root, then the broadcast phase propagates back down.
/// O(log n) messages.
#[derive(Debug)]
pub struct BinaryTreeBarrier {
    rank: usize,
    barrier_mr: BarrierMr,
    poisoned: bool,
}

/// A [`BinaryTreeBarrier`] that has been allocated but not yet linked to remote peers.
#[derive(Debug)]
pub struct PreparedBinaryTreeBarrier {
    rank: usize,
    barrier_mr: PreparedBarrierMr,
}

impl PreparedBinaryTreeBarrier {
    /// Returns this node's barrier memory region handle for exchange with peers.
    pub fn remote(&self) -> PeerRemoteMemoryRegion {
        self.barrier_mr.remote()
    }

    /// Links remote peer memory regions and returns a ready-to-use [`BinaryTreeBarrier`].
    pub fn link_remote(self, remote_mrs: Box<[PeerRemoteMemoryRegion]>) -> BinaryTreeBarrier {
        BinaryTreeBarrier {
            rank: self.rank,
            barrier_mr: self.barrier_mr.link_remote(remote_mrs),
            poisoned: false,
        }
    }
}

impl BinaryTreeBarrier {
    /// Allocates a new binary tree barrier.
    pub fn new(
        pd: &ProtectionDomain,
        rank: usize,
        world_size: usize,
    ) -> IbvResult<PreparedBinaryTreeBarrier> {
        Ok(PreparedBinaryTreeBarrier {
            rank,
            barrier_mr: BarrierMr::new(pd, rank, world_size)?,
        })
    }
}

impl BinaryTreeBarrier {
    /// Synchronizes with the given peers, blocking until all have reached the barrier or timeout.
    ///
    /// Validates that peers are sorted, unique, and include this node's rank.
    pub fn barrier(
        &mut self,
        multi_channel: &mut MultiChannel,
        peers: &[usize],
        timeout: Duration,
    ) -> Result<(), BarrierError> {
        if !peers.is_sorted() {
            return Err(BarrierError::UnorderedPeers);
        }

        if peers.windows(2).any(|w| w[0] == w[1]) {
            return Err(BarrierError::DuplicatePeers);
        }

        self.barrier_unchecked(multi_channel, peers, timeout)
    }

    /// Like [`barrier`](Self::barrier), but skips validation of the peer list.
    pub fn barrier_unchecked(
        &mut self,
        multi_channel: &mut MultiChannel,
        peers: &[usize],
        timeout: Duration,
    ) -> Result<(), BarrierError> {
        if self.poisoned {
            return Err(BarrierError::Poisoned);
        }

        let result = self.run_barrier(multi_channel, peers, timeout);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn run_barrier(
        &mut self,
        multi_channel: &mut MultiChannel,
        peers: &[usize],
        timeout: Duration,
    ) -> Result<(), BarrierError> {
        if peers.len() < 2 {
            return Ok(());
        }

        let start_time = Instant::now();

        let idx = peers
            .binary_search(&self.rank)
            .map_err(|_| BarrierError::SelfNotInGroup)?;

        let parent_rank = (idx > 0).then(|| peers[(idx - 1) / 2]);

        let mut children_ranks_buffer = [0; 2];
        let mut count = 0;
        for child_idx in [2 * idx + 1, 2 * idx + 2] {
            if let Some(&r) = peers.get(child_idx) {
                children_ranks_buffer[count] = r;
                count += 1;
            }
        }
        let children_ranks = &children_ranks_buffer[..count];

        // 1. Notify upwards
        // 1.1 Wait for children
        for &child_rank in children_ranks {
            self.barrier_mr.increase_peer_expected_epoch(child_rank);
            self.barrier_mr
                .spin_poll_peer_epoch_expected(child_rank, start_time, timeout)?;
        }
        // 1.2 Notify parent
        if let Some(parent_rank) = parent_rank {
            self.barrier_mr.notify_peer(multi_channel, parent_rank)?;
        }

        // 2. Notify downwards
        // 2.1 Wait for parent
        if let Some(parent_rank) = parent_rank {
            self.barrier_mr.increase_peer_expected_epoch(parent_rank);
            self.barrier_mr
                .spin_poll_peer_epoch_expected(parent_rank, start_time, timeout)?;
        }
        //2.2 Notify children
        self.barrier_mr
            .scatter_notify_peers(multi_channel, children_ranks)?;

        Ok(())
    }
}
