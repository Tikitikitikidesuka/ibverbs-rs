use crate::channel::multi_channel::MultiChannel;
use crate::channel::multi_channel::rank_work_request::RankWriteWorkRequest;
use crate::channel::raw_channel::pending_work::WorkSpinPollResult;
use crate::ibverbs::remote_memory_region::{RemoteMemorySlice, RemoteMemorySliceMut};
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use crate::ibverbs::work_request::{
    ReadWorkRequest, ReceiveWorkRequest, SendWorkRequest, WriteWorkRequest,
};
use std::borrow::{Borrow, BorrowMut};

impl MultiChannel {
    pub fn send<'a, E, WR>(&'a mut self, peer: usize, wr: WR) -> WorkSpinPollResult
    where
        E: AsRef<[GatherElement<'a>]>,
        WR: Borrow<SendWorkRequest<'a, E>>,
    {
        self.channel(peer)?.send(wr)
    }

    pub fn receive<'a, E, WR>(&'a mut self, peer: usize, wr: WR) -> WorkSpinPollResult
    where
        E: AsMut<[ScatterElement<'a>]>,
        WR: BorrowMut<ReceiveWorkRequest<'a, E>>,
    {
        self.channel(peer)?.receive(wr)
    }

    pub fn write<'a, E, R, WR>(&'a mut self, mut wr: WR) -> WorkSpinPollResult
    where
        E: AsRef<[GatherElement<'a>]>,
        R: BorrowMut<RemoteMemorySliceMut<'a>>,
        WR: BorrowMut<RankWriteWorkRequest<'a, E, R>>,
    {
        let wr = wr.borrow_mut();
        self.channel(wr.peer)?.write(&mut wr.wr)
    }

    pub fn read<'a, E, R, WR>(&'a mut self, peer: usize, wr: WR) -> WorkSpinPollResult
    where
        E: AsMut<[ScatterElement<'a>]>,
        R: Borrow<RemoteMemorySlice<'a>>,
        WR: BorrowMut<ReadWorkRequest<'a, E, R>>,
    {
        self.channel(peer)?.read(wr)
    }
}
