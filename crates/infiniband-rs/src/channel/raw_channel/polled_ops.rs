use crate::channel::raw_channel::RawChannel;
use crate::channel::raw_channel::pending_work::WorkSpinPollResult;
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use crate::ibverbs::work_request::{ReceiveWorkRequest, SendWorkRequest, WriteWorkRequest};
use std::borrow::{Borrow, BorrowMut};

impl RawChannel {
    pub fn send<'a, E, WR>(&'a mut self, wr: WR) -> WorkSpinPollResult
    where
        E: AsRef<[GatherElement<'a>]>,
        WR: Borrow<SendWorkRequest<'a, E>>,
    {
        let res = self.scope(|s| s.post_send(wr)?.spin_poll());
        debug_assert!(
            res.is_ok(),
            "unreachable scope error (single WR, manual poll)"
        );
        res.unwrap()
    }

    pub fn receive<'a, E, WR>(&'a mut self, wr: WR) -> WorkSpinPollResult
    where
        E: AsMut<[ScatterElement<'a>]>,
        WR: BorrowMut<ReceiveWorkRequest<'a, E>>,
    {
        let res = self.scope(|s| s.post_receive(wr)?.spin_poll());
        debug_assert!(
            res.is_ok(),
            "unreachable scope error (single WR, manual poll)"
        );
        res.unwrap()
    }

    pub fn write(&'_ mut self, wr: WriteWorkRequest<'_, '_>) -> WorkSpinPollResult {
        let res = self.scope(|s| s.post_write(wr)?.spin_poll());
        debug_assert!(
            res.is_ok(),
            "unreachable scope error (single WR, manual poll)"
        );
        res.unwrap()
    }

    /*
    pub fn read<'a, E, R, WR>(&'a mut self, wr: WR) -> WorkSpinPollResult
    where
        E: AsMut<[ScatterElement<'a>]>,
        R: Borrow<RemoteMemorySlice<'a>>,
        WR: BorrowMut<ReadWorkRequest<'a, E, R>>,
    {
        let res = self.scope(|s| s.post_read(wr)?.spin_poll());
        debug_assert!(
            res.is_ok(),
            "unreachable scope error (single WR, manual poll)"
        );
        res.unwrap()
    }
    */
}
