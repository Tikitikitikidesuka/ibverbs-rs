use crate::channel::raw_channel::RawChannel;
use crate::channel::raw_channel::pending_work::WorkSpinPollResult;
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};

impl RawChannel {
    pub fn send<'a>(&'a mut self, sends: impl AsRef<[ScatterElement<'a>]>) -> WorkSpinPollResult {
        let res = self.scope(|s| s.post_send(sends)?.spin_poll());
        debug_assert!(
            res.is_ok(),
            "unreachable scope error (single WR, manual poll)"
        );
        res.unwrap()
    }

    pub fn send_with_immediate<'a>(
        &'a mut self,
        sends: impl AsRef<[ScatterElement<'a>]>,
        imm_data: u32,
    ) -> WorkSpinPollResult {
        let res = self.scope(|s| s.post_send_with_immediate(sends, imm_data)?.spin_poll());
        debug_assert!(
            res.is_ok(),
            "unreachable scope error (single WR, manual poll)"
        );
        res.unwrap()
    }

    pub fn receive<'a>(
        &'a mut self,
        receives: impl AsMut<[GatherElement<'a>]>,
    ) -> WorkSpinPollResult {
        let res = self.scope(|s| s.post_receive(receives)?.spin_poll());
        debug_assert!(
            res.is_ok(),
            "unreachable scope error (single WR, manual poll)"
        );
        res.unwrap()
    }
}
