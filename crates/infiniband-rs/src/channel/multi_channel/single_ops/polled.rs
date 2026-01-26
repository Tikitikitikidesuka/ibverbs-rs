use crate::channel::multi_channel::MultiChannel;
use crate::channel::raw_channel::pending_work::WorkSpinPollResult;
use crate::ibverbs::scatter_gather_element::{ScatterElement, GatherElement};

impl MultiChannel {
    pub fn send<'a>(
        &'a mut self,
        peer: usize,
        sends: impl AsRef<[GatherElement<'a>]>,
    ) -> WorkSpinPollResult {
        self.channel(peer)?.send(sends)
    }

    pub fn send_with_immediate<'a>(
        &'a mut self,
        peer: usize,
        sends: impl AsRef<[GatherElement<'a>]>,
        imm_data: u32,
    ) -> WorkSpinPollResult {
        self.channel(peer)?.send_with_immediate(sends, imm_data)
    }

    pub fn send_immediate(&mut self, peer: usize, imm_data: u32) -> WorkSpinPollResult {
        self.channel(peer)?.send_immediate(imm_data)
    }

    pub fn receive<'a>(
        &'a mut self,
        peer: usize,
        receives: impl AsMut<[ScatterElement<'a>]>,
    ) -> WorkSpinPollResult {
        self.channel(peer)?.receive(receives)
    }

    pub fn receive_immediate(
        &mut self,
        peer: usize,
    ) -> WorkSpinPollResult {
        self.channel(peer)?.receive_immediate()
    }
}
