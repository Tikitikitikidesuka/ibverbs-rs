use crate::ibverbs::memory_region::MemoryRegion;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::multi_channel::MultiChannel;
use crate::multi_channel::remote_memory_region::PeerRemoteMemoryRegion;
use crate::multi_channel::work_request::PeerWriteWorkRequest;
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
    #[error("Self not in the barrier group")]
    SelfNotInGroup,
    #[error("Peers not in ascending order in barrier group")]
    UnorderedPeers,
    #[error("Duplicate peers in barrier group")]
    DuplicatePeers,
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
    remote_mrs: Box<[PeerRemoteMemoryRegion]>,
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
    pub fn remote_mr(&self) -> PeerRemoteMemoryRegion {
        PeerRemoteMemoryRegion::new(self.rank, self.mr.remote())
    }

    pub fn link_remote(self, remote_mrs: Box<[PeerRemoteMemoryRegion]>) -> CentralizedBarrier {
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
    const TIMEOUT_CHECK_ITERS: u32 = 1 << 16;

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

        if !peers.contains(&self.rank) {
            return Err(BarrierError::SelfNotInGroup);
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
            self.scatter_notify_peers(multi_channel, &peers[1..])
                .map_err(|error| {
                    self.poisoned = true;
                    error
                })?;
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
            .sub_region(in_epoch_bytes_offset)
            .unwrap();
        let wr = PeerWriteWorkRequest::new(&local_out_epoch_sges, remote_in_epoch_slice);

        // Ensure change is visible before issuing the write
        fence(Ordering::Release);

        // 3. Post RDMA request to notify
        multi_channel.write(wr)?;

        Ok(())
    }

    /// To notify a peer:
    /// 1. The local out epoch counter is increased by one.
    /// 2. The local out epoch counter is RDMA written into the peer's in epoch counter.
    fn scatter_notify_peers(
        &mut self,
        multi_channel: &mut MultiChannel,
        peers: &[usize],
    ) -> io::Result<()> {
        // 1. Increment local epochs
        peers.iter().for_each(|&peer| {
            let current_out_epoch = self.memory[peer].out_epoch.get();
            self.memory[peer].out_epoch.set(current_out_epoch + 1);
        });

        // 2. Prepare SGEs
        // Stored in a Vec to keep the memory alive while WRs reference it
        let sges: Vec<_> = peers
            .iter()
            .map(|&peer| {
                let local_out_epoch_bytes = self.memory[peer].out_epoch.as_bytes();
                [self
                    .mr
                    .prepare_gather_element(local_out_epoch_bytes)
                    .unwrap()]
            })
            .collect();

        // 3. Prepare Remote Slices
        let local_in_epoch_bytes = self.memory[self.rank].in_epoch.as_bytes().as_ptr();
        let in_epoch_bytes_offset = local_in_epoch_bytes as usize - self.memory.as_ptr() as usize;

        // todo: update to new all shared remote memory region
        // We use a raw pointer to mint mutable references.
        // This bypasses the borrow checker's inability to see that `peers` indices are distinct.
        let base_ptr = self.remote_mrs.as_mut_ptr();

        let remote_slices: Vec<_> = peers
            .iter()
            .map(|&peer| {
                // SAFETY:
                // 1. `peers` are sorted and unique (guaranteed by barrier logic)
                // 2. `base_ptr` is valid for the lifetime of `self`
                // 3. We are accessing distinct elements, so the mutable borrows do not overlap
                unsafe {
                    let rmr = &mut *base_ptr.add(peer);
                    rmr.sub_region(in_epoch_bytes_offset).unwrap()
                }
            })
            .collect();

        // 4. Create Work Requests
        // Use .iter() on sges to borrow (not move) the SGE arrays
        let wrs: Vec<_> = sges
            .iter()
            .zip(remote_slices.into_iter())
            .map(|(sge, rms)| PeerWriteWorkRequest::new(sge, rms))
            .collect();

        // Ensure change is visible before issuing the write
        fence(Ordering::Release);

        // 5. Post RDMA request to notify
        multi_channel.scatter_write(wrs)?;

        Ok(())
    }

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
