use crate::channel::raw_channel::RawChannel;
use crate::channel::raw_channel::pending_work::WorkSpinPollResult;
use crate::ibverbs::work_request::{
    ReadWorkRequest, ReceiveWorkRequest, SendWorkRequest, WriteWorkRequest,
};

impl RawChannel {
    pub fn send<'op>(&'op mut self, wr: SendWorkRequest<'op, 'op>) -> WorkSpinPollResult {
        let res = self.scope(|s| s.post_send(wr)?.spin_poll());
        debug_assert!(
            res.is_ok(),
            "unreachable scope error (single WR, manual poll)"
        );
        res.unwrap()
    }

    pub fn receive<'op>(&'op mut self, wr: ReceiveWorkRequest<'op, 'op>) -> WorkSpinPollResult {
        let res = self.scope(|s| s.post_receive(wr)?.spin_poll());
        debug_assert!(
            res.is_ok(),
            "unreachable scope error (single WR, manual poll)"
        );
        res.unwrap()
    }

    pub fn write<'op>(&'op mut self, wr: WriteWorkRequest<'op, 'op>) -> WorkSpinPollResult {
        let res = self.scope(|s| s.post_write(wr)?.spin_poll());
        debug_assert!(
            res.is_ok(),
            "unreachable scope error (single WR, manual poll)"
        );
        res.unwrap()
    }

    pub fn read<'op>(&'op mut self, wr: ReadWorkRequest<'op, 'op>) -> WorkSpinPollResult {
        let res = self.scope(|s| s.post_read(wr)?.spin_poll());
        debug_assert!(
            res.is_ok(),
            "unreachable scope error (single WR, manual poll)"
        );
        res.unwrap()
    }
}
