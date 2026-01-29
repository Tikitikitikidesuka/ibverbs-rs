use crate::ibverbs::remote_memory_region::RemoteMemoryRegion;
use serde::{Deserialize, Serialize};
use std::ops::RangeBounds;

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct PeerRemoteMemoryRegion {
    peer: usize,
    pub(super) remote_mr: RemoteMemoryRegion,
}

impl PeerRemoteMemoryRegion {
    pub(crate) fn new(peer: usize, remote_mr: RemoteMemoryRegion) -> Self {
        Self { peer, remote_mr }
    }

    pub fn peer(&self) -> usize {
        self.peer
    }

    pub fn sub_region(&self, range: impl RangeBounds<usize>) -> Option<PeerRemoteMemoryRegion> {
        Some(PeerRemoteMemoryRegion {
            peer: self.peer,
            remote_mr: self.remote_mr.sub_region(range)?,
        })
    }
}
