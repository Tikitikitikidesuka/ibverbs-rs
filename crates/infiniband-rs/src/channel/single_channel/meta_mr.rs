use crate::ibverbs::memory_region::{MemoryRegion, MemoryRegionEndpoint};
use crate::ibverbs::remote_memory_region::RemoteMemoryRegion;
use bytemuck::{Pod, Zeroable};
use std::fmt::Debug;

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct MetaMrState {
    sync: u32,
    in_msg_ack: u32,
    out_msg_ack: u32,
    _pad: u32,
    in_message: MetaMessage,
    out_message: MetaMessage,
}

impl MetaMr {
    pub fn sync_sge(&self) {
        self.mr.prepare_gather_element(self.memory.as)
    }
}

pub struct MetaMr {
    memory: Box<MetaMrState>,
    mr: MemoryRegion,
    remote_mr: RemoteMemoryRegion,
}

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
