use crate::channel::TransportResult;
use crate::ibverbs::error::IbvResult;
use crate::ibverbs::memory::MemoryRegion;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::multi_channel::MultiChannel;
use crate::multi_channel::{PeerRemoteMemoryRegion, PeerWriteWorkRequest};
use crate::network::barrier::BarrierError;
use crate::remote_struct_array_field_unchecked;
use std::time::{Duration, Instant};
use zerocopy::little_endian::U64;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

/// RDMA memory region used for barrier synchronization.
///
/// Each peer has a slot with epoch counters. A node signals it has reached the barrier
/// by RDMA-writing its outgoing epoch into the remote peer's incoming epoch slot.
/// Completion is detected by polling the local incoming epoch via volatile reads.
#[derive(Debug)]
pub(super) struct BarrierMr {
    rank: usize,
    memory: Box<[BarrierPeerFlags]>,
    mr: MemoryRegion,
    remote_mrs: Box<[PeerRemoteMemoryRegion]>,
}

/// A [`BarrierMr`] that has been allocated but not yet linked to remote peers.
#[derive(Debug)]
pub(super) struct PreparedBarrierMr {
    rank: usize,
    memory: Box<[BarrierPeerFlags]>,
    mr: MemoryRegion,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, FromBytes, IntoBytes, Immutable, KnownLayout)]
struct BarrierPeerFlags {
    /// Expected epoch value — only written and read locally.
    expected_in_epoch: u64,
    /// Outgoing epoch counter, RDMA-written into the remote peer's `in_epoch` slot.
    out_epoch: U64,
    /// Incoming epoch counter, written by remote peers via RDMA to signal arrival.
    in_epoch: U64,
}

impl BarrierPeerFlags {
    fn new() -> Self {
        Self {
            expected_in_epoch: 0,
            in_epoch: U64::new(0),
            out_epoch: U64::new(0),
        }
    }
}

impl PreparedBarrierMr {
    /// Returns this node's barrier memory region handle for exchange with peers.
    pub(super) fn remote(&self) -> PeerRemoteMemoryRegion {
        PeerRemoteMemoryRegion::new(self.rank, self.mr.remote())
    }

    /// Links remote peer memory regions and returns a ready-to-use [`BarrierMr`].
    pub(super) fn link_remote(self, remote_mrs: Box<[PeerRemoteMemoryRegion]>) -> BarrierMr {
        BarrierMr {
            rank: self.rank,
            memory: self.memory,
            mr: self.mr,
            remote_mrs,
        }
    }
}

impl BarrierMr {
    /// Allocates and registers the barrier memory region.
    pub(super) fn new(
        pd: &ProtectionDomain,
        rank: usize,
        world_size: usize,
    ) -> IbvResult<PreparedBarrierMr> {
        let mut memory = vec![BarrierPeerFlags::new(); world_size].into_boxed_slice();
        let memory_bytes = memory.as_mut_bytes();
        let mr = unsafe { pd.register_shared_mr(memory_bytes.as_mut_ptr(), memory_bytes.len())? };
        Ok(PreparedBarrierMr { rank, memory, mr })
    }
}

impl BarrierMr {
    /// Increments the outgoing epoch for `peer` and RDMA-writes it into the peer's incoming slot.
    pub(super) fn notify_peer(
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

    /// Like [`notify_peer`](Self::notify_peer), but notifies multiple peers in a single scatter write.
    pub(super) fn scatter_notify_peers(
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

    /// Busy-waits until the peer's incoming epoch reaches the expected value, or timeout.
    pub(super) fn spin_poll_peer_epoch_expected(
        &mut self,
        peer: usize,
        start_time: Instant,
        timeout: Duration,
    ) -> Result<(), BarrierError> {
        let mut iter = 0u32;

        loop {
            if self.is_peer_epoch_expected(peer) {
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

    /// Increments the expected incoming epoch for the given peer.
    pub(super) fn increase_peer_expected_epoch(&mut self, peer: usize) {
        self.memory[peer].expected_in_epoch += 1;
    }

    /// Returns `true` if the peer's incoming epoch has reached the expected value.
    ///
    /// Uses `>=` rather than `==` because chaining multiple barriers can cause the
    /// incoming epoch to advance past the expected value before it is read.
    pub(super) fn is_peer_epoch_expected(&mut self, peer: usize) -> bool {
        // IMPORTANT: do not change to `==` — see doc comment above.
        unsafe { std::ptr::read_volatile(&self.memory[peer].in_epoch) }.get()
            >= self.memory[peer].expected_in_epoch
    }
}
