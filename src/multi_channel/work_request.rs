use crate::ibverbs::memory::{GatherElement, ScatterElement};
use crate::ibverbs::work::*;
use crate::multi_channel::remote_memory_region::PeerRemoteMemoryRegion;

/// A [`SendWorkRequest`] targeted at a specific peer.
#[derive(Debug, Clone)]
pub struct PeerSendWorkRequest<'wr, 'data> {
    pub(crate) peer: usize,
    pub(crate) wr: SendWorkRequest<'wr, 'data>,
}

/// A [`ReceiveWorkRequest`] targeted at a specific peer.
#[derive(Debug)]
pub struct PeerReceiveWorkRequest<'wr, 'data> {
    pub(crate) peer: usize,
    pub(crate) wr: ReceiveWorkRequest<'wr, 'data>,
}

/// A [`WriteWorkRequest`] targeted at a specific peer.
#[derive(Debug, Clone)]
pub struct PeerWriteWorkRequest<'wr, 'data> {
    pub(crate) peer: usize,
    pub(crate) wr: WriteWorkRequest<'wr, 'data>,
}

/// A [`ReadWorkRequest`] targeted at a specific peer.
#[derive(Debug)]
pub struct PeerReadWorkRequest<'wr, 'data> {
    pub(crate) peer: usize,
    pub(crate) wr: ReadWorkRequest<'wr, 'data>,
}

impl<'wr, 'data> PeerSendWorkRequest<'wr, 'data> {
    /// Creates a new send work request for the given peer.
    pub fn new(peer: usize, gather_elements: &'wr [GatherElement<'data>]) -> Self {
        Self {
            peer,
            wr: SendWorkRequest::new(gather_elements),
        }
    }

    /// Wraps an existing [`SendWorkRequest`] with a peer index.
    pub fn from_wr(peer: usize, wr: SendWorkRequest<'wr, 'data>) -> Self {
        Self { peer, wr }
    }

    /// Attaches an immediate data value to this send.
    pub fn with_immediate(mut self, imm_data: u32) -> Self {
        self.wr = self.wr.with_immediate(imm_data);
        self
    }

    /// Creates a send that carries only immediate data and no payload.
    pub fn only_immediate(peer: usize, imm_data: u32) -> Self {
        Self {
            peer,
            wr: SendWorkRequest::only_immediate(imm_data),
        }
    }

    /// Returns the target peer index.
    pub fn peer(&self) -> usize {
        self.peer
    }
}

impl<'wr, 'data> From<PeerSendWorkRequest<'wr, 'data>> for SendWorkRequest<'wr, 'data> {
    fn from(value: PeerSendWorkRequest<'wr, 'data>) -> Self {
        value.wr
    }
}

impl<'wr, 'data> PeerReceiveWorkRequest<'wr, 'data> {
    /// Creates a new receive work request for the given peer.
    pub fn new(peer: usize, scatter_elements: &'wr mut [ScatterElement<'data>]) -> Self {
        Self {
            peer,
            wr: ReceiveWorkRequest::new(scatter_elements),
        }
    }

    /// Creates a receive that expects only immediate data and no payload.
    pub fn only_immediate(peer: usize) -> Self {
        Self {
            peer,
            wr: ReceiveWorkRequest::only_immediate(),
        }
    }

    /// Wraps an existing [`ReceiveWorkRequest`] with a peer index.
    pub fn from_wr(peer: usize, wr: ReceiveWorkRequest<'wr, 'data>) -> Self {
        Self { peer, wr }
    }

    /// Returns the target peer index.
    pub fn peer(&self) -> usize {
        self.peer
    }
}

impl<'wr, 'data> From<PeerReceiveWorkRequest<'wr, 'data>> for ReceiveWorkRequest<'wr, 'data> {
    fn from(value: PeerReceiveWorkRequest<'wr, 'data>) -> Self {
        value.wr
    }
}

impl<'wr, 'data> PeerWriteWorkRequest<'wr, 'data> {
    /// Creates a new RDMA write work request. The peer is determined by the [`PeerRemoteMemoryRegion`].
    pub fn new(
        gather_elements: &'wr [GatherElement<'data>],
        peer_remote_mr: PeerRemoteMemoryRegion,
    ) -> Self {
        Self {
            peer: peer_remote_mr.peer(),
            wr: WriteWorkRequest::new(gather_elements, peer_remote_mr.remote_mr),
        }
    }

    /// Attaches an immediate data value to this write.
    pub fn with_immediate(mut self, imm_data: u32) -> Self {
        self.wr = self.wr.with_immediate(imm_data);
        self
    }

    /// Returns the target peer index.
    pub fn peer(&self) -> usize {
        self.peer
    }
}

impl<'wr, 'data> From<PeerWriteWorkRequest<'wr, 'data>> for WriteWorkRequest<'wr, 'data> {
    fn from(value: PeerWriteWorkRequest<'wr, 'data>) -> Self {
        value.wr
    }
}

impl<'wr, 'data> PeerReadWorkRequest<'wr, 'data> {
    /// Creates a new RDMA read work request. The peer is determined by the [`PeerRemoteMemoryRegion`].
    pub fn new(
        scatter_elements: &'wr mut [ScatterElement<'data>],
        peer_remote_mr: PeerRemoteMemoryRegion,
    ) -> Self {
        Self {
            peer: peer_remote_mr.peer(),
            wr: ReadWorkRequest::new(scatter_elements, peer_remote_mr.remote_mr),
        }
    }

    /// Returns the target peer index.
    pub fn peer(&self) -> usize {
        self.peer
    }
}

impl<'wr, 'data> From<PeerReadWorkRequest<'wr, 'data>> for ReadWorkRequest<'wr, 'data> {
    fn from(value: PeerReadWorkRequest<'wr, 'data>) -> Self {
        value.wr
    }
}
