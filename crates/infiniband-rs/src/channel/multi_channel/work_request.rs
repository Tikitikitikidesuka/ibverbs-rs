use crate::channel::multi_channel::remote_memory_region::PeerRemoteMemoryRegion;
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use crate::ibverbs::work_request::WriteWorkRequest;

pub struct PeerWriteWorkRequest<'wr, 'data> {
    pub(super) peer: usize,
    pub(super) wr: WriteWorkRequest<'wr, 'data>,
}

/*
pub struct RankReadWorkRequest<'a, E, R>
where
    E: AsMut<[ScatterElement<'a>]>,
    R: Borrow<RemoteMemorySlice<'a>>,
{
    pub(super) peer: usize,
    pub(super) wr: ReadWorkRequest<'a, E, R>,
}
*/

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
}

/*
impl<'a, E, R> RankReadWorkRequest<'a, E, R>
where
    E: AsMut<[ScatterElement<'a>]>,
    R: BorrowMut<RankRemoteMemorySlice<'a>> + BorrowMut<RemoteMemorySlice<'a>>,
{
    pub fn new(scatter_elements: E, remote_slice: R) -> Self {
        let peer = {
            let prs: &RankRemoteMemorySlice = remote_slice.borrow();
            prs.peer
        };

        Self {
            peer,
            wr: ReadWorkRequest::new(scatter_elements, remote_slice),
        }
    }
}
*/
