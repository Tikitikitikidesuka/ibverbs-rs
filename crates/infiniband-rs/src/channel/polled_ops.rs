use crate::channel::Channel;
use crate::channel::pending_work::WorkSpinPollResult;
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};

impl Channel {
    pub fn send<'a>(&mut self, sends: impl AsRef<[ScatterElement<'a>]>) -> WorkSpinPollResult {
        Ok(unsafe { self.send_unpolled(sends)? }.spin_poll()?)
    }

    pub fn send_with_immediate<'a>(
        &mut self,
        sends: impl AsRef<[ScatterElement<'a>]>,
        imm_data: u32,
    ) -> WorkSpinPollResult {
        Ok(unsafe { self.send_with_immediate_unpolled(sends, imm_data)? }.spin_poll()?)
    }

    pub fn receive<'a>(&mut self, receives: impl AsMut<[GatherElement<'a>]>) -> WorkSpinPollResult {
        Ok(unsafe { self.receive_unpolled(receives)? }.spin_poll()?)
    }
}
