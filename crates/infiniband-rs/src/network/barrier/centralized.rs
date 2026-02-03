use crate::ibverbs::error::IbvResult;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::multi_channel::MultiChannel;
use crate::multi_channel::remote_memory_region::PeerRemoteMemoryRegion;
use crate::network::barrier::BarrierError;
use crate::network::barrier::memory::{BarrierMem, PreparedBarrierMem};
use std::time::{Duration, Instant};
use zerocopy::network_endian::U64;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

#[derive(Debug)]
pub struct CentralizedBarrier {
    rank: usize,
    mem: BarrierMem,
    poisoned: bool,
}

#[derive(Debug)]
pub struct PreparedCentralizedBarrier {
    rank: usize,
    mem: PreparedBarrierMem,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, FromBytes, IntoBytes, Immutable, KnownLayout)]
struct CentralizedBarrierPeerFlags {
    in_epoch: U64,
    out_epoch: U64,
}

impl CentralizedBarrierPeerFlags {
    pub fn new() -> Self {
        Self {
            in_epoch: U64::new(0),
            out_epoch: U64::new(0),
        }
    }
}

impl PreparedCentralizedBarrier {
    pub fn remote(&self) -> PeerRemoteMemoryRegion {
        self.mem.remote()
    }

    pub fn link_remote(self, remote_mrs: Box<[PeerRemoteMemoryRegion]>) -> CentralizedBarrier {
        CentralizedBarrier {
            rank: self.rank,
            mem: self.mem.link_remote(remote_mrs),
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
            mem: BarrierMem::new(pd, rank, world_size)?,
        })
    }
}

impl CentralizedBarrier {
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
        if peers.len() < 2 {
            return Ok(());
        }

        let start_time = Instant::now();

        let leader = peers[0];

        if self.rank == leader {
            for &peer in &peers[1..] {
                self.mem
                    .spin_poll_peer_epoch_ahead(peer, start_time, timeout)
                    .map_err(|error| {
                        self.poisoned = true;
                        error
                    })?;
            }
            self.mem
                .scatter_notify_peers(multi_channel, &peers[1..])
                .map_err(|error| {
                    self.poisoned = true;
                    error
                })?;
        } else {
            // If notify leader fails the resulting state is
            self.mem
                .notify_peer(multi_channel, leader)
                .map_err(|error| {
                    self.poisoned = true;
                    error
                })?;
            self.mem
                .spin_poll_peer_same_epoch(leader, start_time, timeout)
                .map_err(|error| {
                    self.poisoned = true;
                    error
                })?;
        }

        Ok(())
    }
}
