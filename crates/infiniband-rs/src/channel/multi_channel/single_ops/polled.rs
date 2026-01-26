use crate::channel::multi_channel::MultiChannel;
use crate::channel::raw_channel::pending_work::WorkSpinPollResult;
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use crate::ibverbs::work_request::{ReceiveWorkRequest, SendWorkRequest};
use std::borrow::{Borrow, BorrowMut};

impl MultiChannel {
    pub fn send<'a, E, WR>(&'a mut self, peer: usize, wr: WR) -> WorkSpinPollResult
    where
        E: AsRef<[GatherElement<'a>]>,
        WR: Borrow<SendWorkRequest<'a, E>>,
    {
        self.channel(peer)?.send(wr)
    }

    pub fn receive<'a, E, WR>(&'a mut self, peer: usize, wr: WR) -> WorkSpinPollResult
    where
        E: AsMut<[ScatterElement<'a>]>,
        WR: BorrowMut<ReceiveWorkRequest<'a, E>>,
    {
        self.channel(peer)?.receive(wr)
    }
}
