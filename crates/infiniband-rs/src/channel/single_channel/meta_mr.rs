use crate::ibverbs::memory_region::MemoryRegion;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::ibverbs::remote_memory_region::{RemoteMemoryRegion, RemoteMemorySliceMut};
use crate::ibverbs::scatter_gather_element::GatherElement;
use crate::ibverbs::work_request::WriteWorkRequest;
use std::fmt::Debug;
use std::mem::offset_of;
use std::sync::atomic::{Ordering, fence};
use std::{io, slice};

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
    in_sync_epoch: u32,
    out_sync_epoch: u32,
    //in_msg_ack: u32,
    //out_msg_ack: u32,
    //_pad: u32,
    //in_message: MetaMessage,
    //out_message: MetaMessage,
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

impl MetaMr {
    pub fn new(pd: &ProtectionDomain) -> io::Result<PreparedMetaMr> {
        let mut memory = Box::new(MetaMrState {
            in_sync_epoch: 0,
            out_sync_epoch: 0,
        });
        let mr = unsafe {
            pd.register_shared_mr(
                memory.as_mut() as *mut MetaMrState as *mut u8,
                size_of::<MetaMrState>(),
            )?
        };

        Ok(PreparedMetaMr { memory, mr })
    }

    pub fn increase_sync_epoch(&mut self) {
        self.memory.out_sync_epoch += 1;
    }

    pub fn get_sync_epoch(&self) -> u32 {
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
                &self.memory.out_sync_epoch as *const u32 as *const u8,
                size_of::<u32>(),
            )
        };

        // To in_sync
        let offset = offset_of!(MetaMrState, in_sync_epoch);
        let range = offset..offset + size_of::<u32>();

        WriteWorkRequest::new(
            vec![self.mr.prepare_gather_element(out_sync_bytes).unwrap()],
            self.remote_mr.slice_mut(range).unwrap(),
        )
    }
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
