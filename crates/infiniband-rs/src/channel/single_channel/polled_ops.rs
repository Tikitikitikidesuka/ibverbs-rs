use crate::channel::raw_channel::pending_work::WorkSpinPollResult;
use crate::channel::single_channel::SingleChannel;
use crate::ibverbs::scatter_gather_element::{ScatterElement, GatherElement};

impl SingleChannel {
    pub fn send<'a>(&'a mut self, sends: impl AsRef<[GatherElement<'a>]>) -> WorkSpinPollResult {
        self.channel.send(sends)
    }

    pub fn send_with_immediate<'a>(
        &'a mut self,
        sends: impl AsRef<[GatherElement<'a>]>,
        imm_data: u32,
    ) -> WorkSpinPollResult {
        self.channel.send_with_immediate(sends, imm_data)
    }

    pub fn send_immediate(&mut self, imm_data: u32) -> WorkSpinPollResult {
        self.channel.send_immediate(imm_data)
    }

    pub fn receive<'a>(
        &'a mut self,
        receives: impl AsMut<[ScatterElement<'a>]>,
    ) -> WorkSpinPollResult {
        self.channel.receive(receives)
    }

    pub fn receive_immediate(&mut self) -> WorkSpinPollResult {
        self.channel.receive_immediate()
    }
}
