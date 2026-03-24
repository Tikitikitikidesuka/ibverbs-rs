use crate::ibverbs::error::IbvResult;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::multi_channel::MultiChannel;
use crate::multi_channel::PeerRemoteMemoryRegion;
use crate::network::barrier::BarrierError;
use crate::network::barrier::memory::{BarrierMr, PreparedBarrierMr};
use std::time::{Duration, Instant};

/// Dissemination barrier implementation.
///
/// In each round, every node notifies a peer at exponentially increasing distance
/// and waits for a notification from the symmetric peer. O(log n) rounds with no
/// designated leader.
#[derive(Debug)]
pub struct DisseminationBarrier {
    rank: usize,
    barrier_mr: BarrierMr,
    poisoned: bool,
}

/// A [`DisseminationBarrier`] that has been allocated but not yet linked to remote peers.
#[derive(Debug)]
pub struct PreparedDisseminationBarrier {
    rank: usize,
    barrier_mr: PreparedBarrierMr,
}

impl PreparedDisseminationBarrier {
    /// Returns this node's barrier memory region handle for exchange with peers.
    pub fn remote(&self) -> PeerRemoteMemoryRegion {
        self.barrier_mr.remote()
    }

    /// Links remote peer memory regions and returns a ready-to-use [`DisseminationBarrier`].
    pub fn link_remote(self, remote_mrs: Box<[PeerRemoteMemoryRegion]>) -> DisseminationBarrier {
        DisseminationBarrier {
            rank: self.rank,
            barrier_mr: self.barrier_mr.link_remote(remote_mrs),
            poisoned: false,
        }
    }
}

impl DisseminationBarrier {
    /// Allocates a new dissemination barrier.
    pub fn new(
        pd: &ProtectionDomain,
        rank: usize,
        world_size: usize,
    ) -> IbvResult<PreparedDisseminationBarrier> {
        Ok(PreparedDisseminationBarrier {
            rank,
            barrier_mr: BarrierMr::new(pd, rank, world_size)?,
        })
    }

    /// Synchronizes with the given peers, blocking until all have reached the barrier or timeout.
    ///
    /// Validates that peers are sorted and unique. Self-inclusion is verified inside the
    /// synchronization itself and returns [`BarrierError::SelfNotInGroup`] if absent.
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

        let len = peers.len();
        let mut distance = 1;

        while distance < len {
            let right_idx = (idx + distance) % len;
            let left_idx = (idx + len - distance) % len;

            let right_rank = peers[right_idx];
            let left_rank = peers[left_idx];

            // 1. Notify the peer to the right
            self.barrier_mr.notify_peer(multi_channel, right_rank)?;

            // 2. Wait for the peer to the left
            self.barrier_mr.increase_peer_expected_epoch(left_rank);
            self.barrier_mr
                .spin_poll_peer_epoch_expected(left_rank, start_time, timeout)?;

            distance *= 2;
        }

        Ok(())
    }
}
