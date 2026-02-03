use crate::ibverbs::error::IbvResult;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::multi_channel::MultiChannel;
use crate::multi_channel::remote_memory_region::PeerRemoteMemoryRegion;
use crate::network::barrier::BarrierError;
use crate::network::barrier::memory::{BarrierMr, PreparedBarrierMr};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct CentralizedBarrier {
    rank: usize,
    barrier_mr: BarrierMr,
    poisoned: bool,
}

#[derive(Debug)]
pub struct PreparedCentralizedBarrier {
    rank: usize,
    barrier_mr: PreparedBarrierMr,
}

impl PreparedCentralizedBarrier {
    pub fn remote(&self) -> PeerRemoteMemoryRegion {
        self.barrier_mr.remote()
    }

    pub fn link_remote(self, remote_mrs: Box<[PeerRemoteMemoryRegion]>) -> CentralizedBarrier {
        CentralizedBarrier {
            rank: self.rank,
            barrier_mr: self.barrier_mr.link_remote(remote_mrs),
            poisoned: false,
        }
    }
}

impl CentralizedBarrier {
    pub fn new(
        pd: &ProtectionDomain,
        rank: usize,
        world_size: usize,
    ) -> IbvResult<PreparedCentralizedBarrier> {
        Ok(PreparedCentralizedBarrier {
            rank,
            barrier_mr: BarrierMr::new(pd, rank, world_size)?,
        })
    }

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

    /// Assumes peers are ordered, non repeating and self is in the group
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

    pub fn run_barrier(
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
                    .map_err(|error| {
                        self.poisoned = true;
                        error
                    })?;
            }
            self.barrier_mr
                .scatter_notify_peers(multi_channel, &peers[1..])
                .map_err(|error| {
                    self.poisoned = true;
                    error
                })?;
        } else {
            // If notify leader fails the resulting state is
            self.barrier_mr
                .notify_peer(multi_channel, leader)
                .map_err(|error| {
                    self.poisoned = true;
                    error
                })?;
            self.barrier_mr
                .spin_poll_peer_epoch_expected(leader, start_time, timeout)
                .map_err(|error| {
                    self.poisoned = true;
                    error
                })?;
        }

        Ok(())
    }
}
