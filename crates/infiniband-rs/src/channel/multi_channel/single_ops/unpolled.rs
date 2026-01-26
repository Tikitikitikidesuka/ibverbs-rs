use crate::channel::multi_channel::MultiChannel;
use crate::channel::raw_channel::pending_work::PendingWork;
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use crate::ibverbs::work_request::{ReceiveWorkRequest, SendWorkRequest};
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
}
