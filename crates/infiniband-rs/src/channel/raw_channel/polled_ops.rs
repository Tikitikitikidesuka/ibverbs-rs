use crate::channel::raw_channel::RawChannel;
use crate::channel::raw_channel::pending_work::WorkSpinPollResult;
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use crate::ibverbs::work_request::{ReceiveWorkRequest, SendWorkRequest};
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

    /*
    pub fn write<'a>(&'a mut self, wr: &mut WriteWorkRequest<'_, 'a>) -> WorkSpinPollResult {
        let res = self.scope(|s| s.post_write(wr)?.spin_poll());
        debug_assert!(
            res.is_ok(),
            "unreachable scope error (single WR, manual poll)"
        );
        res.unwrap()
    }

    pub fn read<'a>(
        &'a mut self,
        scatter_elements: impl AsMut<[ScatterElement<'a>]>,
        remote_slice: &RemoteMemorySlice<'a>,
    ) -> WorkSpinPollResult {
        let res = self.scope(|s| s.post_read(scatter_elements, remote_slice)?.spin_poll());
        debug_assert!(
            res.is_ok(),
            "unreachable scope error (single WR, manual poll)"
        );
        res.unwrap()
    }
    */
}
