use crate::channel::multi_channel::MultiChannel;
use crate::channel::multi_channel::rank_remote_memory_region::RankRemoteMemoryRegion;
use crate::channel::multi_channel::rank_work_request::RankWriteWorkRequest;
use crate::channel::raw_channel::pending_work::MultiWorkPollError;
use crate::ibverbs::memory_region::MemoryRegion;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::ibverbs::remote_memory_region::RemoteMemoryRegion;
use std::borrow::Borrow;
use std::io;
use std::sync::atomic::{Ordering, fence};
use thiserror::Error;
use zerocopy::network_endian::U64;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

#[derive(Debug, Error)]
pub enum BarrierError {
    #[error("Self not in issued barrier's peers")]
    SelfNotInGroup,
    #[error("Network error: {0}")]
    NetworkError(#[from] MultiWorkPollError),
}

#[derive(Debug)]
pub struct CentralizedBarrier {
    rank: usize,
    memory: Box<[CentralizedBarrierPeerFlags]>,
    mr: MemoryRegion,
    remote_mrs: Box<[RankRemoteMemoryRegion]>,
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
    pub fn barrier<I>(&mut self, multi_channel: &mut MultiChannel, peers: I) -> io::Result<()>
    where
        I: IntoIterator<Item = usize>,
        I::IntoIter: ExactSizeIterator,
    {
        // Minimum
        todo!()
    }

    /// Assumes peers are ordered and non repeating
    pub fn barrier_unchecked(
        &mut self,
        multi_channel: &mut MultiChannel,
        peers: &[usize],
    ) -> io::Result<()> {
        if peers.len() < 2 {
            return Ok(());
        }

        let leader = peers[0];

        if self.rank == leader {
            for &peer in &peers[1..] {
                self.await_peer_next_epoch(peer)?;
            }
            for &peer in &peers[1..] {
                self.notify_peer(multi_channel, peer)?;
            }
        } else {
            self.notify_peer(multi_channel, leader)?;
            self.await_peer_same_epoch(leader)?;
        }

        Ok(())
    }

    /// To notify a peer:
    /// 1. The local out epoch counter is increased by one.
    /// 2. The local out epoch counter is RDMA written into the peer's in epoch counter.
    fn leader_notify_peer(&mut self) -> io::Result<()> {
        // 1. Local epoch counter increased by one
        todo!()
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

    fn await_peer_same_epoch(&mut self, peer: usize) -> io::Result<()> {
        loop {
            // 0. Poll in epoch (be -> native)
            let current_in_epoch =
                unsafe { std::ptr::read_volatile(&self.memory[peer].in_epoch) }.get();

            // 1. Wait until the incoming epoch matches the outgoing epoch
            println!("Current in: {current_in_epoch}");
            println!("Current out: {}", self.memory[peer].out_epoch.get());
            if current_in_epoch == self.memory[peer].out_epoch.get() {
                return Ok(());
            }
        }
    }

    fn await_peer_next_epoch(&mut self, peer: usize) -> io::Result<()> {
        loop {
            // 0. Poll in epoch (be -> native)
            let current_in_epoch =
                unsafe { std::ptr::read_volatile(&self.memory[peer].in_epoch) }.get();

            // 1. Wait until the incoming epoch matches the outgoing epoch
            if current_in_epoch > self.memory[peer].out_epoch.get() {
                return Ok(());
            }
        }
    }
}

/*
impl Node {
    pub fn centralized_barrier<I>(
        &mut self,
        peers: impl AsRef<[usize]>,
    ) -> Result<(), BarrierError> {
        let peers = peers.as_ref();

        if !peers.contains(&self.rank) {
            return Err(BarrierError::SelfNotInGroup);
        }

        // Contains self so it is not empty (guaranteed min)
        let coordinator = *peers.iter().min().unwrap();

        if self.rank == coordinator {
            let self_rank = self.rank;
            self.coordinator_centralized_barrier(peers.iter().copied().filter(|&p| p != self_rank))
        } else {
            self.participant_centralized_barrier(coordinator)
        }
    }

    fn coordinator_centralized_barrier(
        &mut self,
        participants: impl Iterator<Item=usize> + Clone,
    ) -> Result<(), BarrierError> {
        // Wait for all participants
        self.gather_immediate(participants.clone())?
            .iter()
            .all(|wc| wc.immediate_data() == Some(Self::PARTICIPANT_READY));

        // Notify all participants
        self.multicast_with_immediate(participants, &[], Self::COORDINATOR_READY)?;

        Ok(())
    }

    fn participant_centralized_barrier(&self, coordinator: usize) -> Result<(), BarrierError> {
        todo!()
        // Notify coordinator
        //self.send_immediate(coordinator, Self::PARTICIPANT_READY);
        /// :( -> This only works if specific channel for this like Alberto did
        /// or back to the memory write read method from my previous implementation

        // Wait for coordinator
    }

    const PARTICIPANT_READY: u32 = 432982347;
    const COORDINATOR_READY: u32 = 958729371;
}
*/
