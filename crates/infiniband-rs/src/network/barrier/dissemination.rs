use crate::ibverbs::error::IbvResult;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::multi_channel::MultiChannel;
use crate::multi_channel::remote_memory_region::PeerRemoteMemoryRegion;
use crate::network::barrier::BarrierError;
use crate::network::barrier::memory::{BarrierMr, PreparedBarrierMr};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct DisseminationBarrier {
    rank: usize,
    barrier_mr: BarrierMr,
    poisoned: bool,
}

#[derive(Debug)]
pub struct PreparedDisseminationBarrier {
    rank: usize,
    barrier_mr: PreparedBarrierMr,
}

impl PreparedDisseminationBarrier {
    pub fn remote(&self) -> PeerRemoteMemoryRegion {
        self.barrier_mr.remote()
    }

    pub fn link_remote(self, remote_mrs: Box<[PeerRemoteMemoryRegion]>) -> DisseminationBarrier {
        DisseminationBarrier {
            rank: self.rank,
            barrier_mr: self.barrier_mr.link_remote(remote_mrs),
            poisoned: false,
        }
    }
}

impl DisseminationBarrier {
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
