use crate::channel::multi_channel::MultiChannel;
use crate::channel::multi_channel::work_request::{
    PeerReadWorkRequest, PeerReceiveWorkRequest, PeerSendWorkRequest, PeerWriteWorkRequest,
};
use crate::channel::raw_channel::pending_work::WorkSpinPollResult;
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use crate::ibverbs::work_request::{
    ReadWorkRequest, ReceiveWorkRequest, SendWorkRequest, WriteWorkRequest,
};
use std::borrow::{Borrow, BorrowMut};

impl MultiChannel {
    pub fn send<'op>(&'op mut self, wr: PeerSendWorkRequest<'op, 'op>) -> WorkSpinPollResult {
        self.channel(wr.peer)?.send(wr.wr)
    }

    pub fn receive<'op>(&'op mut self, wr: PeerReceiveWorkRequest<'op, 'op>) -> WorkSpinPollResult {
        self.channel(wr.peer)?.receive(wr.wr)
    }

    pub fn write<'op>(&'op mut self, wr: PeerWriteWorkRequest<'op, 'op>) -> WorkSpinPollResult {
        self.channel(wr.peer)?.write(wr.wr)
    }

    pub fn read<'op>(&'op mut self, wr: PeerReadWorkRequest<'op, 'op>) -> WorkSpinPollResult {
        self.channel(wr.peer)?.read(wr.wr)
    }
}
