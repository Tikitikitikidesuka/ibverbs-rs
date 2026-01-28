use crate::channel::multi_channel::rank_remote_memory_region::{
    RankRemoteMemorySlice, RankRemoteMemorySliceMut,
};
use crate::ibverbs::remote_memory_region::{RemoteMemorySlice, RemoteMemorySliceMut};
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use crate::ibverbs::work_request::{ReadWorkRequest, WriteWorkRequest};
use std::borrow::{Borrow, BorrowMut};

pub struct RankWriteWorkRequest<'a, E, R>
where
    E: AsRef<[GatherElement<'a>]>,
    R: BorrowMut<RemoteMemorySliceMut<'a>>,
{
    pub(super) peer: usize,
    pub(super) wr: WriteWorkRequest<'a, E, R>,
}

pub struct RankReadWorkRequest<'a, E, R>
where
    E: AsMut<[ScatterElement<'a>]>,
    R: Borrow<RemoteMemorySlice<'a>>,
{
    pub(super) peer: usize,
    pub(super) wr: ReadWorkRequest<'a, E, R>,
}

impl<'a, E, R> RankWriteWorkRequest<'a, E, R>
where
    E: AsRef<[GatherElement<'a>]>,
    R: BorrowMut<RankRemoteMemorySliceMut<'a>> + BorrowMut<RemoteMemorySliceMut<'a>>,
{
    pub fn new(gather_elements: E, mut remote_slice: R) -> Self {
        let peer = {
            let prs: &mut RankRemoteMemorySliceMut = remote_slice.borrow_mut();
            prs.peer
        };

        Self {
            peer,
            wr: WriteWorkRequest::new(gather_elements, remote_slice),
        }
    }

    pub fn with_immediate(mut self, imm_data: u32) -> Self {
        self.wr = self.wr.with_immediate(imm_data);
        self
    }
}

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
