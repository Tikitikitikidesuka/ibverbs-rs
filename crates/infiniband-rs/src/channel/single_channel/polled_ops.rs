use crate::channel::raw_channel::pending_work::WorkSpinPollResult;
use crate::channel::single_channel::SingleChannel;
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use crate::ibverbs::work_request::{ReceiveWorkRequest, SendWorkRequest};
use std::borrow::{Borrow, BorrowMut};

impl SingleChannel {
    pub fn send<'a, E, WR>(&'a mut self, wr: WR) -> WorkSpinPollResult
    where
        E: AsRef<[GatherElement<'a>]>,
        WR: Borrow<SendWorkRequest<'a, E>>,
    {
        self.channel.send(wr)
    }

    pub fn receive<'a, E, WR>(&'a mut self, wr: WR) -> WorkSpinPollResult
    where
        E: AsMut<[ScatterElement<'a>]>,
        WR: BorrowMut<ReceiveWorkRequest<'a, E>>,
    {
        self.channel.receive(wr)
    }
}
