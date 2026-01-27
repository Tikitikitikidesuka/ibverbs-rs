use crate::channel::raw_channel::pending_work::WorkSpinPollResult;
use crate::channel::single_channel::SingleChannel;
use crate::ibverbs::remote_memory_region::{RemoteMemorySlice, RemoteMemorySliceMut};
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use crate::ibverbs::work_request::{
    ReadWorkRequest, ReceiveWorkRequest, SendWorkRequest, WriteWorkRequest,
};
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

    pub fn write<'a, E, R, WR>(&'a mut self, wr: WR) -> WorkSpinPollResult
    where
        E: AsRef<[GatherElement<'a>]>,
        R: BorrowMut<RemoteMemorySliceMut<'a>>,
        WR: BorrowMut<WriteWorkRequest<'a, E, R>>,
    {
        self.channel.write(wr)
    }

    pub fn read<'a, E, R, WR>(&'a mut self, wr: WR) -> WorkSpinPollResult
    where
        E: AsMut<[ScatterElement<'a>]>,
        R: Borrow<RemoteMemorySlice<'a>>,
        WR: BorrowMut<ReadWorkRequest<'a, E, R>>,
    {
        self.channel.read(wr)
    }
}
