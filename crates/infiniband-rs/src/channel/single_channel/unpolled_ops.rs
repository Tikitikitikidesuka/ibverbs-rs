use crate::channel::raw_channel::pending_work::PendingWork;
use crate::channel::single_channel::SingleChannel;
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use crate::ibverbs::work_request::{ReceiveWorkRequest, SendWorkRequest};
use std::io;

impl SingleChannel {
    pub unsafe fn send_unpolled<'a, E: AsRef<[GatherElement<'a>]>>(
        &mut self,
        wr: SendWorkRequest<'a, E>,
    ) -> io::Result<PendingWork<'a>> {
        unsafe { self.channel.send_unpolled(wr) }
    }

    pub unsafe fn receive_unpolled<'a, E: AsMut<[ScatterElement<'a>]>>(
        &mut self,
        wr: ReceiveWorkRequest<'a, E>,
    ) -> io::Result<PendingWork<'a>> {
        unsafe { self.channel.receive_unpolled(wr) }
    }
}
