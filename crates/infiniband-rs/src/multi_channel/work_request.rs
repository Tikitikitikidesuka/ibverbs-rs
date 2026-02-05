use crate::ibverbs::scatter_gather_element::*;
use crate::ibverbs::work_request::*;
use crate::multi_channel::remote_memory_region::PeerRemoteMemoryRegion;

#[derive(Debug, Clone)]
pub struct PeerSendWorkRequest<'wr, 'data> {
    pub(super) peer: usize,
    pub(super) wr: SendWorkRequest<'wr, 'data>,
}

#[derive(Debug)]
pub struct PeerReceiveWorkRequest<'wr, 'data> {
    pub(super) peer: usize,
    pub(super) wr: ReceiveWorkRequest<'wr, 'data>,
}

#[derive(Debug, Clone)]
pub struct PeerWriteWorkRequest<'wr, 'data> {
    pub(super) peer: usize,
    pub(super) wr: WriteWorkRequest<'wr, 'data>,
}

#[derive(Debug)]
pub struct PeerReadWorkRequest<'wr, 'data> {
    pub(super) peer: usize,
    pub(super) wr: ReadWorkRequest<'wr, 'data>,
}

impl<'wr, 'data> PeerSendWorkRequest<'wr, 'data> {
    pub fn new(peer: usize, gather_elements: &'wr [GatherElement<'data>]) -> Self {
        Self {
            peer,
            wr: SendWorkRequest::new(gather_elements),
        }
    }

    pub fn from_wr(peer: usize, wr: SendWorkRequest<'wr, 'data>) -> Self {
        Self { peer, wr }
    }

    pub fn with_immediate(mut self, imm_data: u32) -> Self {
        self.wr = self.wr.with_immediate(imm_data);
        self
    }

    pub fn only_immediate(peer: usize, imm_data: u32) -> Self {
        Self {
            peer,
            wr: SendWorkRequest::only_immediate(imm_data),
        }
    }

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
    pub fn new(peer: usize, scatter_elements: &'wr mut [ScatterElement<'data>]) -> Self {
        Self {
            peer,
            wr: ReceiveWorkRequest::new(scatter_elements),
        }
    }

    pub fn from_wr(peer: usize, wr: ReceiveWorkRequest<'wr, 'data>) -> Self {
        Self { peer, wr }
    }

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
    pub fn new(
        gather_elements: &'wr [GatherElement<'data>],
        peer_remote_mr: PeerRemoteMemoryRegion,
    ) -> Self {
        Self {
            peer: peer_remote_mr.peer(),
            wr: WriteWorkRequest::new(gather_elements, peer_remote_mr.remote_mr),
        }
    }

    pub fn with_immediate(mut self, imm_data: u32) -> Self {
        self.wr = self.wr.with_immediate(imm_data);
        self
    }

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
    pub fn new(
        scatter_elements: &'wr mut [ScatterElement<'data>],
        peer_remote_mr: PeerRemoteMemoryRegion,
    ) -> Self {
        Self {
            peer: peer_remote_mr.peer(),
            wr: ReadWorkRequest::new(scatter_elements, peer_remote_mr.remote_mr),
        }
    }

    pub fn peer(&self) -> usize {
        self.peer
    }
}

impl<'wr, 'data> From<PeerReadWorkRequest<'wr, 'data>> for ReadWorkRequest<'wr, 'data> {
    fn from(value: PeerReadWorkRequest<'wr, 'data>) -> Self {
        value.wr
    }
}
