use crate::ibverbs::memory::RemoteMemoryRegion;
use serde::{Deserialize, Serialize};

/// A wrapper around [`RemoteMemoryRegion`] associated with a specific remote `peer`.
///
/// This struct behaves exactly like `RemoteMemoryRegion` for One-Sided RDMA operations,
/// but carries the destination peer index required to route the operation.
///
/// See [`RemoteMemoryRegion`] for details on RDMA write/read behavior and memory registration.
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct PeerRemoteMemoryRegion {
    peer: usize,
    pub(super) remote_mr: RemoteMemoryRegion,
}

impl PeerRemoteMemoryRegion {
    /// Creates a new `PeerRemoteMemoryRegion` from a peer identifier and a `RemoteMemoryRegion`.
    pub fn new(peer: usize, remote_mr: RemoteMemoryRegion) -> Self {
        Self { peer, remote_mr }
    }

    /// Returns the peer identifier associated with this remote memory region.
    pub fn peer(&self) -> usize {
        self.peer
    }

    /// Delegates to [`RemoteMemoryRegion::sub_region`], returning a new `PeerRemoteMemoryRegion`
    /// tied to the same peer.
    ///
    /// # Returns
    ///
    /// * `Some(PeerRemoteMemoryRegion)` if the offset is within bounds.
    /// * `None` if the offset exceeds the current length.
    pub fn sub_region(&self, offset: usize) -> Option<PeerRemoteMemoryRegion> {
        Some(PeerRemoteMemoryRegion {
            peer: self.peer,
            remote_mr: self.remote_mr.sub_region(offset)?,
        })
    }

    /// Like [`sub_region`](Self::sub_region), but without bounds checking.
    pub fn sub_region_unchecked(&self, offset: usize) -> PeerRemoteMemoryRegion {
        PeerRemoteMemoryRegion {
            peer: self.peer,
            remote_mr: self.remote_mr.sub_region_unchecked(offset),
        }
    }
}
