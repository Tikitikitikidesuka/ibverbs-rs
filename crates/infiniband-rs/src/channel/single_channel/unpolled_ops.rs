use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use std::io;
use crate::channel::raw_channel::pending_work::PendingWork;
use crate::channel::single_channel::SingleChannel;

impl SingleChannel {
    pub unsafe fn send_unpolled<'a>(
        &mut self,
        sends: impl AsRef<[ScatterElement<'a>]>,
    ) -> io::Result<PendingWork<'a>> {
        self.channel.send_unpolled(sends)
    }

    pub unsafe fn send_with_immediate_unpolled<'a>(
        &mut self,
        sends: impl AsRef<[ScatterElement<'a>]>,
        imm_data: u32,
    ) -> io::Result<PendingWork<'a>> {
        self.channel.send_with_immediate_unpolled(sends, imm_data)
    }

    pub unsafe fn receive_unpolled<'a>(
        &mut self,
        mut receives: impl AsMut<[GatherElement<'a>]>,
    ) -> io::Result<PendingWork<'a>> {
        self.channel.receive_unpolled(receives)
    }
}
