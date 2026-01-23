use crate::channel::pending_work::WorkSpinPollResult;
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use crate::single_channel::SingleChannel;

impl SingleChannel {
    pub fn send<'a>(&mut self, sends: impl AsRef<[ScatterElement<'a>]>) -> WorkSpinPollResult {
        self.channel.send(sends)
    }

    pub fn send_with_immediate<'a>(
        &mut self,
        sends: impl AsRef<[ScatterElement<'a>]>,
        imm_data: u32,
    ) -> WorkSpinPollResult {
        self.channel.send_with_immediate(sends, imm_data)
    }

    pub fn receive<'a>(&mut self, receives: impl AsMut<[GatherElement<'a>]>) -> WorkSpinPollResult {
        self.channel.receive(receives)
    }
}
