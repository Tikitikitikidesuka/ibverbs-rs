use crate::channel::pending_work::WorkSpinPollResult;
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use crate::multi_channel::MultiChannel;

impl MultiChannel {
    pub fn send<'a>(
        &mut self,
        peer: usize,
        sends: impl AsRef<[ScatterElement<'a>]>,
    ) -> WorkSpinPollResult {
        self.channel(peer)?.send(sends)
    }

    pub fn send_with_immediate<'a>(
        &mut self,
        peer: usize,
        sends: impl AsRef<[ScatterElement<'a>]>,
        imm_data: u32,
    ) -> WorkSpinPollResult {
        self.channel(peer)?.send_with_immediate(sends, imm_data)
    }

    pub fn receive<'a>(
        &mut self,
        peer: usize,
        receives: impl AsMut<[GatherElement<'a>]>,
    ) -> WorkSpinPollResult {
        self.channel(peer)?.receive(receives)
    }
}
