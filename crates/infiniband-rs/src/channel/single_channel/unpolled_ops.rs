use crate::channel::raw_channel::pending_work::PendingWork;
use crate::channel::single_channel::SingleChannel;
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use crate::ibverbs::work_request::{
    ReadWorkRequest, ReceiveWorkRequest, SendWorkRequest, WriteWorkRequest,
};
use std::borrow::{Borrow, BorrowMut};
use std::io;

impl SingleChannel {
    pub unsafe fn send_unpolled<'data>(
        &mut self,
        wr: SendWorkRequest<'_, 'data>,
    ) -> io::Result<PendingWork<'data>> {
        unsafe { self.channel.send_unpolled(wr) }
    }

    pub unsafe fn receive_unpolled<'data>(
        &mut self,
        wr: ReceiveWorkRequest<'_, 'data>,
    ) -> io::Result<PendingWork<'data>> {
        unsafe { self.channel.receive_unpolled(wr) }
    }

    pub unsafe fn write_unpolled<'data>(
        &mut self,
        wr: WriteWorkRequest<'_, 'data>,
    ) -> io::Result<PendingWork<'data>> {
        unsafe { self.channel.write_unpolled(wr) }
    }

    pub unsafe fn read_unpolled<'data>(
        &mut self,
        wr: ReadWorkRequest<'_, 'data>,
    ) -> io::Result<PendingWork<'data>> {
        unsafe { self.channel.read_unpolled(wr) }
    }
}
