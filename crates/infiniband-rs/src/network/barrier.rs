use crate::channel::multi_channel::MultiChannel;
use crate::channel::multi_channel::rank_remote_memory_region::RankRemoteMemoryRegion;
use crate::channel::multi_channel::rank_work_request::RankWriteWorkRequest;
use crate::ibverbs::memory_region::MemoryRegion;
use crate::ibverbs::protection_domain::ProtectionDomain;
use std::io;
use std::sync::atomic::{Ordering, fence};
use std::time::{Duration, Instant};
use thiserror::Error;
use zerocopy::network_endian::U64;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

#[derive(Debug, Error)]
pub enum BarrierError {
    #[error("Barrier is poisoned from a previous error")]
    Poisoned,
    #[error("Self not in issued barrier's peers")]
    SelfNotInGroup,
    #[error("Peers not in ascending order")]
    UnorderedPeers,
    #[error("Barrier timeout")]
    Timeout,
    #[error("Network error: {0}")]
    NetworkError(#[from] io::Error),
}

#[derive(Debug)]
pub struct CentralizedBarrier {
    rank: usize,
    memory: Box<[CentralizedBarrierPeerFlags]>,
    mr: MemoryRegion,
    remote_mrs: Box<[RankRemoteMemoryRegion]>,
    poisoned: bool,
}

#[derive(Debug)]
pub struct PreparedCentralizedBarrier {
    rank: usize,
    memory: Box<[CentralizedBarrierPeerFlags]>,
    mr: MemoryRegion,
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
    pub fn remote_mr(&self) -> RankRemoteMemoryRegion {
        RankRemoteMemoryRegion::new(self.rank, self.mr.remote())
    }

    pub fn link_remote(self, remote_mrs: Box<[RankRemoteMemoryRegion]>) -> CentralizedBarrier {
        CentralizedBarrier {
            rank: self.rank,
            memory: self.memory,
            mr: self.mr,
            remote_mrs,
            poisoned: false,
        }
    }
}

impl CentralizedBarrier {
    pub fn new(
        pd: &ProtectionDomain,
        rank: usize,
        world_size: usize,
    ) -> io::Result<PreparedCentralizedBarrier> {
        let mut memory = vec![CentralizedBarrierPeerFlags::new(); world_size].into_boxed_slice();

        let memory_bytes = memory.as_mut_bytes();
        let mr = unsafe { pd.register_shared_mr(memory_bytes.as_mut_ptr(), memory_bytes.len())? };

        Ok(PreparedCentralizedBarrier { rank, memory, mr })
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
            panic!("peers must be sorted");
        }

        if peers.windows(2).any(|w| w[0] == w[1]) {
            panic!("peers must not contain duplicates");
        }

        if !peers.contains(&self.rank) {
            panic!("self must be included in peers");
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
        if peers.len() < 2 {
            return Ok(());
        }

        let start_time = Instant::now();

        let leader = peers[0];

        if self.rank == leader {
            for &peer in &peers[1..] {
                self.await_peer_next_epoch(peer, start_time, timeout)
                    .map_err(|error| {
                        self.poisoned = true;
                        error
                    })?;
            }
            for &peer in &peers[1..] {
                self.notify_peer(multi_channel, peer).map_err(|error| {
                    self.poisoned = true;
                    error
                })?;
            }
        } else {
            // If notify leader fails the resulting state is
            self.notify_peer(multi_channel, leader).map_err(|error| {
                self.poisoned = true;
                error
            })?;
            self.await_peer_same_epoch(leader, start_time, timeout)
                .map_err(|error| {
                    self.poisoned = true;
                    error
                })?;
        }

        Ok(())
    }

    /// To notify a peer:
    /// 1. The local out epoch counter is increased by one.
    /// 2. The local out epoch counter is RDMA written into the peer's in epoch counter.
    fn notify_peer(&mut self, multi_channel: &mut MultiChannel, peer: usize) -> io::Result<()> {
        // 1. Local epoch counter increased by one
        let current_out_epoch = self.memory[peer].out_epoch.get();
        self.memory[peer].out_epoch.set(current_out_epoch + 1);

        // 2. Prepare the RDMA write wr to write the local out epoch counter
        // into the peer's in epoch counter.
        let local_out_epoch_bytes = self.memory[peer].out_epoch.as_bytes();
        // Unwrap because the bytes are guaranteed to be in the mr and fit in a sge.
        let local_out_epoch_sges = [self
            .mr
            .prepare_gather_element(local_out_epoch_bytes)
            .unwrap()];
        let local_in_epoch_bytes = self.memory[self.rank].in_epoch.as_bytes().as_ptr();
        let in_epoch_bytes_offset = local_in_epoch_bytes as usize - self.memory.as_ptr() as usize;
        let remote_in_epoch_slice = self.remote_mrs[peer]
            .slice_mut(in_epoch_bytes_offset..(in_epoch_bytes_offset + size_of::<u64>()))
            .unwrap();
        let wr = RankWriteWorkRequest::new(&local_out_epoch_sges, remote_in_epoch_slice);

        // Ensure change is visible before issuing the write
        fence(Ordering::Release);

        // 3. Post RDMA request to notify
        multi_channel.write(wr)?;

        Ok(())
    }

    const TIMEOUT_CHECK_ITERS: u32 = 1000;

    fn await_peer_same_epoch(
        &mut self,
        peer: usize,
        start_time: Instant,
        timeout: Duration,
    ) -> Result<(), BarrierError> {
        let mut iter = 0u32;

        loop {
            if self.is_epoch_same(peer) {
                return Ok(());
            }

            iter += 1;
            if iter >= Self::TIMEOUT_CHECK_ITERS {
                iter = 0;
                if start_time.elapsed() > timeout {
                    return Err(BarrierError::Timeout);
                }
            }
        }
    }

    fn await_peer_next_epoch(
        &mut self,
        peer: usize,
        start_time: Instant,
        timeout: Duration,
    ) -> Result<(), BarrierError> {
        let mut iter = 0u32;

        loop {
            if self.is_peer_epoch_ahead(peer) {
                return Ok(());
            }

            iter += 1;
            if iter >= Self::TIMEOUT_CHECK_ITERS {
                iter = 0;
                if start_time.elapsed() > timeout {
                    return Err(BarrierError::Timeout);
                }
            }
        }
    }

    fn is_epoch_same(&self, peer: usize) -> bool {
        unsafe { std::ptr::read_volatile(&self.memory[peer].in_epoch) }.get()
            == self.memory[peer].out_epoch.get()
    }

    fn is_peer_epoch_ahead(&self, peer: usize) -> bool {
        unsafe { std::ptr::read_volatile(&self.memory[peer].in_epoch) }.get()
            > self.memory[peer].out_epoch.get()
    }
}
