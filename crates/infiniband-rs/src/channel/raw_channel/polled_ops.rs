use crate::channel::raw_channel::RawChannel;
use crate::channel::raw_channel::pending_work::WorkSpinPollResult;
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use crate::ibverbs::work_request::{ReceiveWorkRequest, SendWorkRequest};

impl RawChannel {
    pub fn send<'a, E: AsRef<[GatherElement<'a>]>>(
        &'a mut self,
        wr: SendWorkRequest<'a, E>,
    ) -> WorkSpinPollResult {
        let res = self.scope(|s| s.post_send(wr)?.spin_poll());
        debug_assert!(
            res.is_ok(),
            "unreachable scope error (single WR, manual poll)"
        );
        res.unwrap()
    }

    pub fn receive<'a, E: AsMut<[ScatterElement<'a>]>>(
        &'a mut self,
        wr: ReceiveWorkRequest<'a, E>,
    ) -> WorkSpinPollResult {
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
