use crate::channel::pending_work::{PendingWork, WorkSpinPollResult};
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use crate::multi_channel::MultiChannel;
use std::io;

impl MultiChannel {
    pub unsafe fn send_unpolled<'a>(
        &mut self,
        peer: usize,
        sends: impl AsRef<[ScatterElement<'a>]>,
    ) -> io::Result<PendingWork<'a>> {
        unsafe { self.channel(peer)?.send_unpolled(sends.as_ref()) }
    }

    pub unsafe fn send_with_immediate_unpolled<'a>(
        &mut self,
        peer: usize,
        sends: impl AsRef<[ScatterElement<'a>]>,
        imm_data: u32,
    ) -> io::Result<PendingWork<'a>> {
        unsafe {
            self.channel(peer)?
                .send_with_immediate_unpolled(sends.as_ref(), imm_data)
        }
    }

    pub unsafe fn receive_unpolled<'a>(
        &mut self,
        peer: usize,
        mut receives: impl AsMut<[GatherElement<'a>]>,
    ) -> io::Result<PendingWork<'a>> {
        unsafe { self.channel(peer)?.receive_unpolled(receives.as_mut()) }
    }
}
