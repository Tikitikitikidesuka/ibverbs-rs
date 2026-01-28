use crate::channel::multi_channel::MultiChannel;
use crate::channel::multi_channel::rank_work_request::RankWriteWorkRequest;
use crate::channel::raw_channel::pending_work::PendingWork;
use crate::ibverbs::remote_memory_region::{RemoteMemorySlice, RemoteMemorySliceMut};
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use crate::ibverbs::work_request::{
    ReadWorkRequest, ReceiveWorkRequest, SendWorkRequest, WriteWorkRequest,
};
use std::borrow::{Borrow, BorrowMut};
use std::io;

impl MultiChannel {
    pub unsafe fn send_unpolled<'a, E, WR>(
        &mut self,
        peer: usize,
        wr: WR,
    ) -> io::Result<PendingWork<'a>>
    where
        E: AsRef<[GatherElement<'a>]>,
        WR: Borrow<SendWorkRequest<'a, E>>,
    {
        unsafe { self.channel(peer)?.send_unpolled(wr) }
    }

    pub unsafe fn receive_unpolled<'a, E, WR>(
        &mut self,
        peer: usize,
        wr: WR,
    ) -> io::Result<PendingWork<'a>>
    where
        E: AsMut<[ScatterElement<'a>]>,
        WR: BorrowMut<ReceiveWorkRequest<'a, E>>,
    {
        unsafe { self.channel(peer)?.receive_unpolled(wr) }
    }

    pub unsafe fn write_unpolled<'a, E, R, WR>(&mut self, mut wr: WR) -> io::Result<PendingWork<'a>>
    where
        E: AsRef<[GatherElement<'a>]>,
        R: BorrowMut<RemoteMemorySliceMut<'a>>,
        WR: BorrowMut<RankWriteWorkRequest<'a, E, R>>,
    {
        let wr = wr.borrow_mut();
        unsafe { self.channel(wr.peer)?.write_unpolled(&mut wr.wr) }
    }

    pub unsafe fn read_unpolled<'a, E, R, WR>(
        &mut self,
        peer: usize,
        wr: WR,
    ) -> io::Result<PendingWork<'a>>
    where
        E: AsMut<[ScatterElement<'a>]>,
        R: Borrow<RemoteMemorySlice<'a>>,
        WR: BorrowMut<ReadWorkRequest<'a, E, R>>,
    {
        unsafe { self.channel(peer)?.read_unpolled(wr) }
    }
}
