use crate::ibverbs::memory_region::MemoryRegion;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::ibverbs::remote_memory_region::{RemoteMemoryRegion, RemoteMemorySliceMut};
use crate::ibverbs::scatter_gather_element::GatherElement;
use crate::ibverbs::work_request::WriteWorkRequest;
use std::borrow::BorrowMut;
use std::fmt::Debug;
use std::mem::{MaybeUninit, offset_of};
use std::sync::atomic::{Ordering, fence};
use std::{io, slice};
use thiserror::Error;

pub struct MetaMr {
    memory: Box<MetaMrState>,
    mr: MemoryRegion,
    remote_mr: RemoteMemoryRegion,
}

pub struct PreparedMetaMr {
    memory: Box<MetaMrState>,
    mr: MemoryRegion,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MetaMrState {
    in_sync_epoch: usize,
    out_sync_epoch: usize,

    // --- Incoming Write Section (Written by Peer) ---
    pub in_remote_mr: MaybeUninit<RemoteMemoryRegion>, // Incoming remote mr
    pub peer_remote_mr_epoch: usize,                   // Number of remote mrs received

    pub local_remote_mr_ack: usize, // Number of remote mrs acknowledged by peer

    // --- Outgoing Write Section (Written by Local) ---
    pub out_remote_mr: MaybeUninit<RemoteMemoryRegion>, // Outgoing remote mr
    pub local_remote_mr_epoch: usize,                   // Number of remote mrs sent

    pub peer_remote_mr_ack: usize, // Number of remote mrs acknowledged to peer
}

impl PreparedMetaMr {
    pub fn remote(&self) -> RemoteMemoryRegion {
        self.mr.remote()
    }

    pub fn link_remote(self, remote_mr: RemoteMemoryRegion) -> MetaMr {
        MetaMr {
            memory: self.memory,
            mr: self.mr,
            remote_mr,
        }
    }
}

#[derive(Debug, Error)]
pub enum MetaMrError {
    #[error("Missing ack from peer")]
    PendingAck,
    #[error(transparent)]
    IoError(#[from] io::Error),
}

impl MetaMr {
    pub fn new(pd: &ProtectionDomain) -> io::Result<PreparedMetaMr> {
        let mut memory = Box::new(MetaMrState {
            in_sync_epoch: 0,
            out_sync_epoch: 0,
            in_remote_mr: MaybeUninit::uninit(),
            out_remote_mr: MaybeUninit::uninit(),
            local_remote_mr_epoch: 0,
            local_remote_mr_ack: 0,
            peer_remote_mr_epoch: 0,
            peer_remote_mr_ack: 0,
        });
        let mr = unsafe {
            pd.register_shared_mr(
                memory.as_mut() as *mut MetaMrState as *mut u8,
                size_of::<MetaMrState>(),
            )?
        };

        Ok(PreparedMetaMr { memory, mr })
    }

    pub fn prepare_write_remote_mr_wr(
        &mut self,
        remote_mr: RemoteMemoryRegion,
    ) -> Result<WriteWorkRequest<Vec<GatherElement>, RemoteMemorySliceMut>, MetaMrError> {
        // Volatile read to ensure we see DMA updates
        let ack = unsafe { std::ptr::read_volatile(&self.memory.local_remote_mr_ack) };

        // Fence to ensure ordering (Ack check happens before we overwrite data)
        fence(Ordering::Acquire);

        // Check peer has consumed last sent remote mr
        if self.memory.local_remote_mr_epoch > ack {
            return Err(MetaMrError::PendingAck);
        }

        self.memory.out_remote_mr = MaybeUninit::new(remote_mr);
        self.memory.local_remote_mr_epoch += 1;

        let mr_bytes = unsafe {
            slice::from_raw_parts(
                self.memory.out_remote_mr.as_ptr() as *const u8,
                size_of::<RemoteMemoryRegion>(),
            )
        };

        let epoch_bytes = unsafe {
            slice::from_raw_parts(
                &self.memory.local_remote_mr_epoch as *const usize as *const u8,
                size_of::<usize>(),
            )
        };

        let sge_mr = self.mr.prepare_gather_element(mr_bytes).unwrap();
        let sge_epoch = self.mr.prepare_gather_element(epoch_bytes).unwrap();

        let offset = offset_of!(MetaMrState, in_remote_mr);
        let len = size_of::<RemoteMemoryRegion>() + size_of::<usize>();
        let range = offset..offset + len;

        let remote_slice = self.remote_mr.slice_mut(range).ok_or(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Remote slice bounds",
        ))?;

        // Ensure written memory visible by NIC
        fence(Ordering::Release);

        Ok(WriteWorkRequest::new(vec![sge_mr, sge_epoch], remote_slice))
    }

    pub fn read_remote_mr(&self) -> Option<RemoteMemoryRegion> {
        // Volatile read for the epoch written by peer
        let epoch = unsafe { std::ptr::read_volatile(&self.memory.peer_remote_mr_epoch) };

        // Fence to ensure we read the payload (MR) *after* checking the epoch
        fence(Ordering::Acquire);

        // Check if new data is available
        if epoch > self.memory.peer_remote_mr_ack {
            // SAFE: We confirmed epoch updated, so in_remote_mr must be valid
            Some(unsafe { self.memory.in_remote_mr.assume_init() })
        } else {
            None
        }
    }

    pub fn prepare_write_ack_remote_mr_wr(
        &mut self,
    ) -> Result<WriteWorkRequest<Vec<GatherElement>, RemoteMemorySliceMut>, MetaMrError> {
        // Increment the acknowledgement counter
        self.memory.peer_remote_mr_ack += 1;

        let ack_bytes = unsafe {
            slice::from_raw_parts(
                &self.memory.peer_remote_mr_ack as *const usize as *const u8,
                size_of::<usize>(),
            )
        };

        let sge_ack = self.mr.prepare_gather_element(ack_bytes).unwrap();

        let offset = offset_of!(MetaMrState, local_remote_mr_ack);
        let len = size_of::<usize>();
        let range = offset..offset + len;

        let remote_slice = self.remote_mr.slice_mut(range).unwrap();

        // Ensure the update to `peer_remote_mr_ack` is visible to the NIC before it reads it.
        fence(Ordering::Release);

        Ok(WriteWorkRequest::new(vec![sge_ack], remote_slice))
    }

    /*
    pub fn increase_sync_epoch(&mut self) {
        self.memory.out_sync_epoch += 1;
    }

    pub fn get_sync_epoch(&self) -> usize {
        self.memory.in_sync_epoch
    }

    pub fn prepare_sync_epoch_wr(
        &mut self,
    ) -> WriteWorkRequest<Vec<GatherElement>, RemoteMemorySliceMut> {
        // Ensure previous modifications are visible to NIC
        fence(Ordering::Release);

        // Write from out_sync
        let out_sync_bytes: &[u8] = unsafe {
            slice::from_raw_parts(
                &self.memory.out_sync_epoch as *const usize as *const u8,
                size_of::<u32>(),
            )
        };

        // To in_sync
        let offset = offset_of!(MetaMrState, in_sync_epoch);
        let range = offset..offset + size_of::<usize>();

        WriteWorkRequest::new(
            vec![self.mr.prepare_gather_element(out_sync_bytes).unwrap()],
            self.remote_mr.slice_mut(range).unwrap(),
        )
    }
    */
}

/*
#[derive(Debug, Copy, Clone)]
pub enum MetaMessageView {
    SharedMemoryRegion(MemoryRegionEndpoint),
    Unknown(u32),
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct MetaMessage {
    pub tag: u32,
    pub _pad: u32,
    pub payload: MetaMessagePayload,
}

#[repr(C)]
#[derive(Copy, Clone, Zeroable)]
pub union MetaMessagePayload {
    pub shared_mr: MemoryRegionEndpoint,
}

// SAFETY:
// 1. The union is `repr(C)`.
// 2. All fields (MemoryRegionEndpoint) are Pod.
// 3. Use of MetaMessage tag for safe logical access.
unsafe impl Pod for MetaMessagePayload {}

impl MetaMessage {
    pub const TAG_SHARED_MR: u32 = 1;

    pub fn view(&self) -> MetaMessageView {
        // SAFETY: Only access the union field if the tag matches known variants.
        unsafe {
            match self.tag {
                Self::TAG_SHARED_MR => MetaMessageView::SharedMemoryRegion(self.payload.shared_mr),
                t => MetaMessageView::Unknown(t),
            }
        }
    }

    pub fn set(&mut self, val: MetaMessageView) {
        match val {
            MetaMessageView::SharedMemoryRegion(mr) => {
                self.tag = Self::TAG_SHARED_MR;
                self.payload.shared_mr = mr;
            }
            MetaMessageView::Unknown(t) => {
                self.tag = t;
                // Leave payload as is
            }
        }
    }
}

impl Debug for MetaMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.view())
    }
}
 */
