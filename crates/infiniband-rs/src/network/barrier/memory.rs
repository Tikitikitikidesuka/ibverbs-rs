use crate::channel::TransportResult;
use crate::ibverbs::error::IbvResult;
use crate::ibverbs::memory_region::MemoryRegion;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::multi_channel::MultiChannel;
use crate::multi_channel::remote_memory_region::PeerRemoteMemoryRegion;
use crate::multi_channel::work_request::PeerWriteWorkRequest;
use crate::network::barrier::BarrierError;
use crate::remote_struct_array_field_unchecked;
use std::time::{Duration, Instant};
use zerocopy::network_endian::U64;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

#[derive(Debug)]
pub struct BarrierMem {
    rank: usize,
    memory: Box<[BarrierPeerFlags]>,
    mr: MemoryRegion,
    remote_mrs: Box<[PeerRemoteMemoryRegion]>,
}

#[derive(Debug)]
pub struct PreparedBarrierMem {
    rank: usize,
    memory: Box<[BarrierPeerFlags]>,
    mr: MemoryRegion,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, FromBytes, IntoBytes, Immutable, KnownLayout)]
struct BarrierPeerFlags {
    in_epoch: U64,
    out_epoch: U64,
}

impl BarrierPeerFlags {
    pub fn new() -> Self {
        Self {
            in_epoch: U64::new(0),
            out_epoch: U64::new(0),
        }
    }
}

impl PreparedBarrierMem {
    pub fn remote(&self) -> PeerRemoteMemoryRegion {
        PeerRemoteMemoryRegion::new(self.rank, self.mr.remote())
    }

    pub fn link_remote(self, remote_mrs: Box<[PeerRemoteMemoryRegion]>) -> BarrierMem {
        BarrierMem {
            rank: self.rank,
            memory: self.memory,
            mr: self.mr,
            remote_mrs,
        }
    }
}

impl BarrierMem {
    pub fn new(
        pd: &ProtectionDomain,
        rank: usize,
        world_size: usize,
    ) -> IbvResult<PreparedBarrierMem> {
        let mut memory = vec![BarrierPeerFlags::new(); world_size].into_boxed_slice();
        let memory_bytes = memory.as_mut_bytes();
        let mr = unsafe { pd.register_shared_mr(memory_bytes.as_mut_ptr(), memory_bytes.len())? };
        Ok(PreparedBarrierMem { rank, memory, mr })
    }
}

impl BarrierMem {
    // Increases local epoch and writes it to peer
    pub fn notify_peer(
        &mut self,
        multi_channel: &mut MultiChannel,
        peer: usize,
    ) -> TransportResult<()> {
        let current_out_epoch = self.memory[peer].out_epoch.get();
        self.memory[peer].out_epoch.set(current_out_epoch + 1);

        let local_out_epoch_bytes = self.memory[peer].out_epoch.as_bytes();
        let local_out_epoch_sges = [self.mr.gather_element_unchecked(local_out_epoch_bytes)];
        let peer_in_epoch_remote_mr = remote_struct_array_field_unchecked!(
            self.remote_mrs[peer],
            BarrierPeerFlags,
            self.rank,
            in_epoch
        );
        let wr = PeerWriteWorkRequest::new(&local_out_epoch_sges, peer_in_epoch_remote_mr);
        multi_channel.write(wr)?;
        Ok(())
    }

    pub fn scatter_notify_peers(
        &mut self,
        multi_channel: &mut MultiChannel,
        peers: &[usize],
    ) -> TransportResult<()> {
        peers.iter().for_each(|&peer| {
            let current = self.memory[peer].out_epoch.get();
            self.memory[peer].out_epoch.set(current + 1);
        });

        let part_wrs = peers
            .iter()
            .map(|&peer| {
                let local_out_epoch_bytes = self.memory[peer].out_epoch.as_bytes();
                let local_out_epoch_sges =
                    [self.mr.gather_element_unchecked(local_out_epoch_bytes)];
                let peer_in_epoch_remote_mr = remote_struct_array_field_unchecked!(
                    self.remote_mrs[peer],
                    BarrierPeerFlags,
                    self.rank,
                    in_epoch
                );
                (local_out_epoch_sges, peer_in_epoch_remote_mr)
            })
            .collect::<Vec<_>>();
        let wrs = part_wrs
            .iter()
            .map(|(sges, rmr)| PeerWriteWorkRequest::new(sges, *rmr));
        multi_channel.scatter_write(wrs)?;
        Ok(())
    }

    const TIMEOUT_CHECK_ITERS: u32 = 1 << 16;

    pub fn spin_poll_peer_same_epoch(
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

    pub fn spin_poll_peer_epoch_ahead(
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
