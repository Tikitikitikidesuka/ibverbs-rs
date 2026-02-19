use crate::channel::{Channel, TransportError};
use crate::ibverbs::error::IbvResult;
use crate::ibverbs::memory::{MemoryRegion, RemoteMemoryRegion};
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::ibverbs::work::WriteWorkRequest;
use crate::remote_struct_field;
use std::fmt::Debug;
use std::mem::offset_of;
use std::sync::atomic::{Ordering, fence};
use std::time::Duration;
use thiserror::Error;
use zerocopy::network_endian::{U32, U64};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

#[derive(Debug)]
pub struct RemoteMrExchanger {
    memory: Box<RemoteMrExchangerState>,
    mr: MemoryRegion,
    remote_mr: RemoteMemoryRegion,
}

pub struct PreparedRemoteMrExchanger {
    memory: Box<RemoteMrExchangerState>,
    mr: MemoryRegion,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, FromBytes, IntoBytes, Immutable, KnownLayout)]
struct RemoteMrExchangerState {
    in_remote_mr: PodRemoteMemoryRegion,
    in_epoch: U64,
    in_ack: U64,
    out_remote_mr: PodRemoteMemoryRegion,
    out_epoch: U64,
    out_ack: U64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, FromBytes, IntoBytes, Immutable, KnownLayout)]
struct PodRemoteMemoryRegion {
    addr: U64,
    length: U64,
    rkey: U32,
    _pad: U32,
}

// Native -> Big Endian
impl From<RemoteMemoryRegion> for PodRemoteMemoryRegion {
    fn from(value: RemoteMemoryRegion) -> Self {
        PodRemoteMemoryRegion {
            addr: U64::new(value.address()),
            length: U64::new(value.length() as u64),
            rkey: U32::new(value.rkey()),
            _pad: U32::new(0),
        }
    }
}

// Big Endian -> Native
impl From<PodRemoteMemoryRegion> for RemoteMemoryRegion {
    fn from(value: PodRemoteMemoryRegion) -> Self {
        RemoteMemoryRegion::new(
            value.addr.get(),
            value.length.get() as usize,
            value.rkey.get(),
        )
    }
}

impl PodRemoteMemoryRegion {
    fn new() -> Self {
        Self {
            addr: U64::new(0),
            length: U64::new(0),
            rkey: U32::new(0),
            _pad: U32::new(0),
        }
    }
}

impl PreparedRemoteMrExchanger {
    pub fn remote(&self) -> RemoteMemoryRegion {
        self.mr.remote()
    }

    pub fn link_remote(self, remote_mr: RemoteMemoryRegion) -> RemoteMrExchanger {
        RemoteMrExchanger {
            memory: self.memory,
            mr: self.mr,
            remote_mr,
        }
    }
}

#[derive(Debug, Error)]
pub enum RemoteMrExchangerError {
    #[error("Peer has not ACKed the previous shared memory region")]
    SharePeerNotReady,
    #[error("Timeout accepting shared memory region")]
    AcceptTimeout,
    #[error(transparent)]
    TransportError(#[from] TransportError),
}

impl RemoteMrExchanger {
    pub fn new(pd: &ProtectionDomain) -> IbvResult<PreparedRemoteMrExchanger> {
        let mut memory = Box::new(RemoteMrExchangerState {
            in_remote_mr: PodRemoteMemoryRegion::new(),
            in_epoch: U64::new(0),
            in_ack: U64::new(0),
            out_remote_mr: PodRemoteMemoryRegion::new(),
            out_epoch: U64::new(0),
            out_ack: U64::new(0),
        });

        let memory_bytes = memory.as_mut_bytes();
        let mr = unsafe { pd.register_shared_mr(memory_bytes.as_mut_ptr(), memory_bytes.len())? };

        Ok(PreparedRemoteMrExchanger { memory, mr })
    }

    /// Protocol for sharing a remote memory region.
    /// 1. Write the remote memory region to the meta mr with `set_remote_mr`.
    /// 2. Advance the epoch with `increase_remote_mr_epoch`.
    /// 3. RDMA write the wr from `prepare_write_remote_mr_wr`.
    /// This writes the remote mr to the peer.
    /// 4. RDMA write the wr from `prepare_write_remote_mr_epoch_wr`.
    /// This advances the epoch to the peer, notifying him that he can read
    /// the previously sent memory region. These two cannot be done in a single
    /// work request because there is no guarantee that the operation is atomic.
    /// However, a reliable rechannel like RawChannel guarantees that multiple WRs
    /// are seen in order of issuance.
    pub fn share_memory_region(
        &mut self,
        channel: &mut Channel,
        mr: &MemoryRegion,
    ) -> Result<(), RemoteMrExchangerError> {
        // todo: custom error
        // 0. Check the peer acknowledged the last shared remote mr (be -> native)
        let current_in_ack = unsafe { std::ptr::read_volatile(&self.memory.in_ack) }.get();
        if self.memory.out_epoch > current_in_ack {
            return Err(RemoteMrExchangerError::SharePeerNotReady);
        }

        // 1. Write the mr's remote handle to the outgoing remote mr field
        self.memory.out_remote_mr = mr.remote().into();

        // 2. Increase the epoch in the outgoing remote mr epoch field (native -> +1 -> be)
        let new_epoch = self.memory.out_epoch.get() + 1;
        self.memory.out_epoch.set(new_epoch);

        // Slice the meta remote memory region
        // Unwrap because we are taking the full slice
        let in_remote_mr =
            remote_struct_field!(self.remote_mr, RemoteMrExchangerState::in_remote_mr).unwrap();
        let in_epoch =
            remote_struct_field!(self.remote_mr, RemoteMrExchangerState::in_epoch).unwrap();

        // 3. Prepare RDMA write request of the remote mr
        // Get slice of the outgoing remote mr field's bytes
        let out_remote_mr_bytes = self.memory.out_remote_mr.as_bytes();
        let remote_mr_sges = [self.mr.gather_element_unchecked(out_remote_mr_bytes)];
        let remote_mr_wr = WriteWorkRequest::new(&remote_mr_sges, in_remote_mr);

        // 4. Prepare RDMA write request of the remote mr epoch
        // Get slice of the remote mr epoch field's bytes
        let out_epoch_bytes = self.memory.out_epoch.as_bytes();
        let remote_mr_epoch_sges = [self.mr.gather_element_unchecked(out_epoch_bytes)];
        let remote_mr_epoch_wr = WriteWorkRequest::new(&remote_mr_epoch_sges, in_epoch);

        // Ensure changes are visible before issuing the writes
        fence(Ordering::Release);

        // 5. Post RDMA write operations in the correct order:
        // - Firstly write the remote mr.
        // - Secondly write the increased epoch.
        channel.manual_scope(|s| {
            let remote_mr_wr = s.post_write(remote_mr_wr)?;
            let epoch_wr = s.post_write(remote_mr_epoch_wr)?;
            remote_mr_wr.spin_poll()?;
            epoch_wr.spin_poll()?;
            Ok::<(), TransportError>(())
        })?;

        Ok(())
    }

    /// Protocol for receive a shared memory region
    /// 1. Wait until the epoch is one value higher than the ack.
    /// 2. Read the remote memory region.
    /// 3. Acknowledge it by adding one to the ack and RDMA writing the wr generated from
    /// `prepare_write_ack_remote_mr_wr` to the peer that shared the mr.
    pub fn accept_memory_region(
        &mut self,
        channel: &mut Channel,
        timeout: Duration,
    ) -> Result<RemoteMemoryRegion, RemoteMrExchangerError> {
        let start = std::time::Instant::now();

        loop {
            // 0. Poll Epoch (be -> native)
            let current_in_epoch = unsafe { std::ptr::read_volatile(&self.memory.in_epoch) }.get();

            // 1. Wait until the incoming epoch is higher than the one acknowledged
            if current_in_epoch > self.memory.out_ack.get() {
                // 2. Read the remote memory region
                let remote_mr = self.memory.in_remote_mr.into();

                // 3. Increase the outgoing acknowledge counter (native -> +1 -> be)
                let new_epoch = self.memory.out_ack.get() + 1;
                self.memory.out_ack.set(new_epoch);

                // Slice the meta remote memory region
                let ack_offset = offset_of!(RemoteMrExchangerState, in_ack);
                let meta_remote_mr_slice =
                    remote_struct_field!(self.remote_mr, RemoteMrExchangerState::in_ack).unwrap();

                // Get slice of the remote mr ack field's bytes
                let remote_mr_ack_sge = [self
                    .mr
                    .gather_element(self.memory.out_ack.as_bytes())
                    .unwrap()];

                // 4. Prepare RDMA write request of the remote mr ack
                let wr = WriteWorkRequest::new(&remote_mr_ack_sge, meta_remote_mr_slice);

                // Ensure change is visible before issuing the write
                fence(Ordering::Release);

                // 5. Post RDMA write operation to acknowledge.
                channel.write(wr)?;

                return Ok(remote_mr);
            }

            if start.elapsed() > timeout {
                return Err(RemoteMrExchangerError::AcceptTimeout);
            }

            std::hint::spin_loop()
        }
    }
}
