use crate::channel::multi_channel::MultiChannel;
use crate::channel::raw_channel::pending_work::{MultiWorkSpinPollResult, WorkPollError};
use crate::channel::raw_channel::polling_scope::{PollingScope, ScopedPendingWork};
use crate::ibverbs::remote_memory_region::RemoteMemorySliceMut;
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use crate::ibverbs::work_request::{ReceiveWorkRequest, SendWorkRequest, WriteWorkRequest};
use crate::ibverbs::work_success::WorkSuccess;
use std::borrow::{Borrow, BorrowMut};
use std::io;
use crate::channel::multi_channel::rank_work_request::RankWriteWorkRequest;

impl<'scope, 'env> PollingScope<'scope, 'env, MultiChannel> {
    pub fn post_scatter<I, E, WR>(&mut self, wrs: I) -> io::Result<Vec<ScopedPendingWork<'scope>>>
    where
        I: IntoIterator<Item = (usize, WR)>,
        E: AsRef<[GatherElement<'env>]>,
        WR: Borrow<SendWorkRequest<'env, E>>,
    {
        wrs.into_iter()
            .map(|(peer, wr)| self.post_send(peer, wr))
            .collect()
    }

    pub fn post_scatter_write<'a, I, E, R, WR>(
        &mut self,
        wrs: I,
    ) -> io::Result<Vec<ScopedPendingWork<'scope>>>
    where
        I: IntoIterator<Item = WR>,
        E: AsRef<[GatherElement<'env>]>,
        R: BorrowMut<RemoteMemorySliceMut<'env>>,
        WR: BorrowMut<RankWriteWorkRequest<'env, E, R>>,
    {
        wrs.into_iter().map(|wr| self.post_write(wr)).collect()
    }

    pub fn post_gather<I, E, WR>(&mut self, wrs: I) -> io::Result<Vec<ScopedPendingWork<'scope>>>
    where
        I: IntoIterator<Item = (usize, WR)>,
        E: AsMut<[ScatterElement<'env>]>,
        WR: BorrowMut<ReceiveWorkRequest<'env, E>>,
    {
        wrs.into_iter()
            .map(|(peer, wr)| self.post_receive(peer, wr))
            .collect()
    }

    pub fn post_multicast<I, E, WR>(
        &mut self,
        peers: I,
        wr: WR,
    ) -> io::Result<Vec<ScopedPendingWork<'scope>>>
    where
        I: IntoIterator<Item = usize>,
        E: AsRef<[GatherElement<'env>]>,
        WR: Borrow<SendWorkRequest<'env, E>>,
    {
        let wr = wr.borrow();
        peers
            .into_iter()
            .map(|peer| self.post_send(peer, wr))
            .collect::<io::Result<Vec<_>>>()
    }
}
