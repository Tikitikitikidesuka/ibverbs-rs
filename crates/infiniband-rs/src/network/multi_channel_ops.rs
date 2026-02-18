use crate::channel::TransportResult;
use crate::channel::pending_work::PendingWork;
use crate::channel::polling_scope::{PollingScope, ScopeError, ScopedPendingWork};
use crate::ibverbs::error::IbvResult;
use crate::ibverbs::work::{SendWorkRequest, WorkSuccess};
use crate::multi_channel::work_request::*;
use crate::network::Node;
use crate::network::barrier::BarrierError;
use std::time::Duration;

impl Node {
    pub fn scope<'env, F, T>(&'env mut self, f: F) -> Result<T, ScopeError>
    where
        F: for<'scope> FnOnce(&mut PollingScope<'scope, 'env, Node>) -> TransportResult<T>,
    {
        PollingScope::run(self, f)
    }
}

impl<'scope, 'env> PollingScope<'scope, 'env, Node> {
    pub fn post_scatter_send<'wr, I>(&mut self, wrs: I) -> IbvResult<Vec<ScopedPendingWork<'scope>>>
    where
        I: IntoIterator<Item = PeerSendWorkRequest<'wr, 'env>>,
        'env: 'wr,
    {
        wrs.into_iter().map(|wr| self.post_send(wr)).collect()
    }

    pub fn post_scatter_write<'wr, I>(
        &mut self,
        wrs: I,
    ) -> IbvResult<Vec<ScopedPendingWork<'scope>>>
    where
        I: IntoIterator<Item = PeerWriteWorkRequest<'wr, 'env>>,
        'env: 'wr,
    {
        wrs.into_iter().map(|wr| self.post_write(wr)).collect()
    }

    pub fn post_gather_receive<'wr, I>(
        &mut self,
        wrs: I,
    ) -> IbvResult<Vec<ScopedPendingWork<'scope>>>
    where
        I: IntoIterator<Item = PeerReceiveWorkRequest<'wr, 'env>>,
        'env: 'wr,
    {
        wrs.into_iter().map(|wr| self.post_receive(wr)).collect()
    }

    pub fn post_gather_read<'wr, I>(&mut self, wrs: I) -> IbvResult<Vec<ScopedPendingWork<'scope>>>
    where
        I: IntoIterator<Item = PeerReadWorkRequest<'wr, 'env>>,
        'env: 'wr,
    {
        wrs.into_iter().map(|wr| self.post_read(wr)).collect()
    }

    pub fn post_multicast_send<'wr, I>(
        &mut self,
        peers: I,
        wr: SendWorkRequest<'wr, 'env>,
    ) -> IbvResult<Vec<ScopedPendingWork<'scope>>>
    where
        I: IntoIterator<Item = usize>,
        'env: 'wr,
    {
        peers
            .into_iter()
            .map(|peer| self.post_send(PeerSendWorkRequest::from_wr(peer, wr.clone())))
            .collect()
    }
}

impl<'scope, 'env> PollingScope<'scope, 'env, Node> {
    pub fn barrier(&mut self, peers: &[usize], timeout: Duration) -> Result<(), BarrierError> {
        self.inner.barrier(peers, timeout)
    }

    pub fn barrier_unchecked(
        &mut self,
        peers: &[usize],
        timeout: Duration,
    ) -> Result<(), BarrierError> {
        self.inner.barrier_unchecked(peers, timeout)
    }

    pub fn post_send(
        &mut self,
        wr: PeerSendWorkRequest<'_, 'env>,
    ) -> IbvResult<ScopedPendingWork<'scope>> {
        self.channel_post_send(|n| n.multi_channel.channel(wr.peer), wr.wr)
    }

    pub fn post_receive(
        &mut self,
        wr: PeerReceiveWorkRequest<'_, 'env>,
    ) -> IbvResult<ScopedPendingWork<'scope>> {
        self.channel_post_receive(|n| n.multi_channel.channel(wr.peer), wr.wr)
    }

    pub fn post_write(
        &mut self,
        wr: PeerWriteWorkRequest<'_, 'env>,
    ) -> IbvResult<ScopedPendingWork<'scope>> {
        self.channel_post_write(|n| n.multi_channel.channel(wr.peer), wr.wr)
    }

    pub fn post_read(
        &mut self,
        wr: PeerReadWorkRequest<'_, 'env>,
    ) -> IbvResult<ScopedPendingWork<'scope>> {
        self.channel_post_read(|n| n.multi_channel.channel(wr.peer), wr.wr)
    }
}

impl Node {
    pub fn send<'op>(
        &'op mut self,
        wr: PeerSendWorkRequest<'op, 'op>,
    ) -> TransportResult<WorkSuccess> {
        self.multi_channel.send(wr)
    }

    pub fn receive<'op>(
        &'op mut self,
        wr: PeerReceiveWorkRequest<'op, 'op>,
    ) -> TransportResult<WorkSuccess> {
        self.multi_channel.receive(wr)
    }

    pub fn write<'op>(
        &'op mut self,
        wr: PeerWriteWorkRequest<'op, 'op>,
    ) -> TransportResult<WorkSuccess> {
        self.multi_channel.write(wr)
    }

    pub fn read<'op>(
        &'op mut self,
        wr: PeerReadWorkRequest<'op, 'op>,
    ) -> TransportResult<WorkSuccess> {
        self.multi_channel.read(wr)
    }
}

impl Node {
    pub unsafe fn send_unpolled<'data>(
        &mut self,
        wr: PeerSendWorkRequest<'_, 'data>,
    ) -> IbvResult<PendingWork<'data>> {
        unsafe { self.multi_channel.send_unpolled(wr) }
    }

    pub unsafe fn receive_unpolled<'data>(
        &mut self,
        wr: PeerReceiveWorkRequest<'_, 'data>,
    ) -> IbvResult<PendingWork<'data>> {
        unsafe { self.multi_channel.receive_unpolled(wr) }
    }

    pub unsafe fn write_unpolled<'data>(
        &mut self,
        wr: PeerWriteWorkRequest<'_, 'data>,
    ) -> IbvResult<PendingWork<'data>> {
        unsafe { self.multi_channel.write_unpolled(wr) }
    }

    pub unsafe fn read_unpolled<'data>(
        &mut self,
        wr: PeerReadWorkRequest<'_, 'data>,
    ) -> IbvResult<PendingWork<'data>> {
        unsafe { self.multi_channel.read_unpolled(wr) }
    }
}

impl Node {
    pub fn scatter_send<'op, I>(&'op mut self, wrs: I) -> TransportResult<Vec<WorkSuccess>>
    where
        I: IntoIterator<Item = PeerSendWorkRequest<'op, 'op>>,
    {
        self.multi_channel.scatter_send(wrs)
    }

    pub fn scatter_write<'op, I>(&'op mut self, wrs: I) -> TransportResult<Vec<WorkSuccess>>
    where
        I: IntoIterator<Item = PeerWriteWorkRequest<'op, 'op>>,
    {
        self.multi_channel.scatter_write(wrs)
    }

    pub fn gather_receive<'op, I>(&'op mut self, wrs: I) -> TransportResult<Vec<WorkSuccess>>
    where
        I: IntoIterator<Item = PeerReceiveWorkRequest<'op, 'op>>,
    {
        self.multi_channel.gather_receive(wrs)
    }

    pub fn gather_read<'op, I>(&'op mut self, wrs: I) -> TransportResult<Vec<WorkSuccess>>
    where
        I: IntoIterator<Item = PeerReadWorkRequest<'op, 'op>>,
    {
        self.multi_channel.gather_read(wrs)
    }

    pub fn multicast_send<'op, I>(
        &'op mut self,
        peers: I,
        wr: SendWorkRequest<'op, 'op>,
    ) -> TransportResult<Vec<WorkSuccess>>
    where
        I: IntoIterator<Item = usize>,
    {
        self.multi_channel.multicast_send(peers, wr)
    }
}

impl Node {
    pub fn scatter_send_unpolled<'wr, 'data, I>(
        &mut self,
        wrs: I,
    ) -> IbvResult<Vec<PendingWork<'data>>>
    where
        I: IntoIterator<Item = PeerSendWorkRequest<'wr, 'data>>,
        'data: 'wr,
    {
        self.multi_channel.scatter_send_unpolled(wrs)
    }

    pub fn scatter_write_unpolled<'wr, 'data, I>(
        &mut self,
        wrs: I,
    ) -> IbvResult<Vec<PendingWork<'data>>>
    where
        I: IntoIterator<Item = PeerWriteWorkRequest<'wr, 'data>>,
        'data: 'wr,
    {
        self.multi_channel.scatter_write_unpolled(wrs)
    }

    pub fn gather_receive_unpolled<'wr, 'data, I>(
        &mut self,
        wrs: I,
    ) -> IbvResult<Vec<PendingWork<'data>>>
    where
        I: IntoIterator<Item = PeerReceiveWorkRequest<'wr, 'data>>,
        'data: 'wr,
    {
        self.multi_channel.gather_receive_unpolled(wrs)
    }

    pub fn gather_read_unpolled<'wr, 'data, I>(
        &mut self,
        wrs: I,
    ) -> IbvResult<Vec<PendingWork<'data>>>
    where
        I: IntoIterator<Item = PeerReadWorkRequest<'wr, 'data>>,
        'data: 'wr,
    {
        self.multi_channel.gather_read_unpolled(wrs)
    }

    pub fn multicast_send_unpolled<'wr, 'data, I>(
        &mut self,
        peers: I,
        wr: SendWorkRequest<'wr, 'data>,
    ) -> IbvResult<Vec<PendingWork<'data>>>
    where
        I: IntoIterator<Item = usize>,
    {
        self.multi_channel.multicast_send_unpolled(peers, wr)
    }
}
