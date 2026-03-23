use crate::ibverbs::error::IbvResult;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::multi_channel::MultiChannel;
use crate::multi_channel::remote_memory_region::PeerRemoteMemoryRegion;
use crate::network::barrier::BarrierError;
use crate::network::barrier::memory::{BarrierMr, PreparedBarrierMr};
use std::time::{Duration, Instant};

/// Centralized (leader-based) barrier implementation.
///
/// The first peer in the group acts as leader. All other peers notify the leader,
/// then wait for the leader to notify them back. O(n) messages.
#[derive(Debug)]
pub struct LinearBarrier {
    rank: usize,
    barrier_mr: BarrierMr,
    poisoned: bool,
}

/// A [`LinearBarrier`] that has been allocated but not yet linked to remote peers.
#[derive(Debug)]
pub struct PreparedLinearBarrier {
    rank: usize,
    barrier_mr: PreparedBarrierMr,
}

impl PreparedLinearBarrier {
    /// Returns this node's barrier memory region handle for exchange with peers.
    pub fn remote(&self) -> PeerRemoteMemoryRegion {
        self.barrier_mr.remote()
    }

    /// Links remote peer memory regions and returns a ready-to-use [`LinearBarrier`].
    pub fn link_remote(self, remote_mrs: Box<[PeerRemoteMemoryRegion]>) -> LinearBarrier {
        LinearBarrier {
            rank: self.rank,
            barrier_mr: self.barrier_mr.link_remote(remote_mrs),
            poisoned: false,
        }
    }
}

impl LinearBarrier {
    /// Allocates a new linear barrier.
    pub fn new(
        pd: &ProtectionDomain,
        rank: usize,
        world_size: usize,
    ) -> IbvResult<PreparedLinearBarrier> {
        Ok(PreparedLinearBarrier {
            rank,
            barrier_mr: BarrierMr::new(pd, rank, world_size)?,
        })
    }

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

        peers
            .binary_search(&self.rank)
            .map_err(|_| BarrierError::SelfNotInGroup)?;

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

        let leader = peers[0];

        if self.rank == leader {
            for &peer in &peers[1..] {
                self.barrier_mr.increase_peer_expected_epoch(peer);
                self.barrier_mr
                    .spin_poll_peer_epoch_expected(peer, start_time, timeout)
                    .inspect_err(|_error| {
                        self.poisoned = true;
                    })?;
            }
            self.barrier_mr
                .scatter_notify_peers(multi_channel, &peers[1..])
                .inspect_err(|_error| {
                    self.poisoned = true;
                })?;
        } else {
            // If notify leader fails the resulting state is
            self.barrier_mr
                .notify_peer(multi_channel, leader)
                .inspect_err(|_error| {
                    self.poisoned = true;
                })?;
            self.barrier_mr.increase_peer_expected_epoch(leader);
            self.barrier_mr
                .spin_poll_peer_epoch_expected(leader, start_time, timeout)
                .inspect_err(|_error| {
                    self.poisoned = true;
                })?;
        }

        Ok(())
    }
}
