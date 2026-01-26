use crate::channel::raw_channel::pending_work::PendingWork;
use crate::channel::single_channel::SingleChannel;
use crate::ibverbs::scatter_gather_element::{ScatterElement, GatherElement};
use std::io;

impl SingleChannel {
    pub unsafe fn send_unpolled<'a>(
        &mut self,
        sends: impl AsRef<[GatherElement<'a>]>,
    ) -> io::Result<PendingWork<'a>> {
        unsafe { self.channel.send_unpolled(sends) }
    }

    pub unsafe fn send_with_immediate_unpolled<'a>(
        &mut self,
        sends: impl AsRef<[GatherElement<'a>]>,
        imm_data: u32,
    ) -> io::Result<PendingWork<'a>> {
        unsafe { self.channel.send_with_immediate_unpolled(sends, imm_data) }
    }

    pub fn send_immediate_unpolled<'a>(&mut self, imm_data: u32) -> io::Result<PendingWork<'a>> {
        self.channel.send_immediate_unpolled(imm_data)
    }

    pub unsafe fn receive_unpolled<'a>(
        &mut self,
        receives: impl AsMut<[ScatterElement<'a>]>,
    ) -> io::Result<PendingWork<'a>> {
        unsafe { self.channel.receive_unpolled(receives) }
    }

    pub fn receive_immediate_unpolled<'a>(&mut self) -> io::Result<PendingWork<'a>> {
        self.channel.receive_immediate_unpolled()
    }
}
