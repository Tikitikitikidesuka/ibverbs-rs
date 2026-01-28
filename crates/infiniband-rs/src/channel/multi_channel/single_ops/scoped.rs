use crate::channel::multi_channel::MultiChannel;
use crate::channel::raw_channel::pending_work::{MultiWorkPollError, PendingWork};
use crate::channel::raw_channel::polling_scope::{PollingScope, ScopedPendingWork};
use crate::ibverbs::remote_memory_region::{RemoteMemorySlice, RemoteMemorySliceMut};
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use crate::ibverbs::work_request::{
    ReadWorkRequest, ReceiveWorkRequest, SendWorkRequest, WriteWorkRequest,
};
use std::borrow::{Borrow, BorrowMut};
use std::io;
use crate::channel::multi_channel::rank_work_request::RankWriteWorkRequest;

impl MultiChannel {
    pub fn scope<'env, F, R>(&'env mut self, f: F) -> Result<R, MultiWorkPollError>
    where
        F: for<'scope> FnOnce(&mut PollingScope<'scope, 'env, MultiChannel>) -> R,
    {
        PollingScope::run(self, f)
    }
}

impl<'scope, 'env> PollingScope<'scope, 'env, MultiChannel> {
    pub fn post_send<E, WR>(&mut self, peer: usize, wr: WR) -> io::Result<ScopedPendingWork<'scope>>
    where
        E: AsRef<[GatherElement<'env>]>,
        WR: Borrow<SendWorkRequest<'env, E>>,
    {
        self.channel_post_send(|m| m.channel(peer), wr)
    }

    pub fn post_receive<E, WR>(
        &mut self,
        peer: usize,
        wr: WR,
    ) -> io::Result<ScopedPendingWork<'scope>>
    where
        E: AsMut<[ScatterElement<'env>]>,
        WR: BorrowMut<ReceiveWorkRequest<'env, E>>,
    {
        self.channel_post_receive(|m| m.channel(peer), wr)
    }

    pub fn post_write<E, R, WR>(&mut self, mut wr: WR) -> io::Result<ScopedPendingWork<'scope>>
    where
        E: AsRef<[GatherElement<'env>]>,
        R: BorrowMut<RemoteMemorySliceMut<'env>>,
        WR: BorrowMut<RankWriteWorkRequest<'env, E, R>>,
    {
        let wr = wr.borrow_mut();
        self.channel_post_write(|m| m.channel(wr.peer), &mut wr.wr)
    }

    pub fn post_read<E, R, WR>(
        &mut self,
        peer: usize,
        wr: WR,
    ) -> io::Result<ScopedPendingWork<'scope>>
    where
        E: AsMut<[ScatterElement<'env>]>,
        R: Borrow<RemoteMemorySlice<'env>>,
        WR: BorrowMut<ReadWorkRequest<'env, E, R>>,
    {
        self.channel_post_read(|m| m.channel(peer), wr)
    }
}
