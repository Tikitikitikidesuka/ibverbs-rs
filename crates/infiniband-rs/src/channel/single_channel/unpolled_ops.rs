use crate::channel::raw_channel::pending_work::PendingWork;
use crate::channel::single_channel::SingleChannel;
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use crate::ibverbs::work_request::{ReceiveWorkRequest, SendWorkRequest};
use std::borrow::{Borrow, BorrowMut};
use std::io;

impl SingleChannel {
    pub unsafe fn send_unpolled<'a, E, WR>(&mut self, wr: WR) -> io::Result<PendingWork<'a>>
    where
        E: AsRef<[GatherElement<'a>]>,
        WR: Borrow<SendWorkRequest<'a, E>>,
    {
        unsafe { self.channel.send_unpolled(wr) }
    }

    pub unsafe fn receive_unpolled<'a, E, WR>(&mut self, wr: WR) -> io::Result<PendingWork<'a>>
    where
        E: AsMut<[ScatterElement<'a>]>,
        WR: BorrowMut<ReceiveWorkRequest<'a, E>>,
    {
        unsafe { self.channel.receive_unpolled(wr) }
    }
}
