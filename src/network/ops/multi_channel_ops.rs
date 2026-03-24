use crate::channel::TransportResult;
use crate::channel::{PendingWork, PollingScope, ScopedPendingWork};
use crate::ibverbs::error::IbvResult;
use crate::ibverbs::work::{SendWorkRequest, WorkSuccess};
use crate::multi_channel::{
    PeerReadWorkRequest, PeerReceiveWorkRequest, PeerSendWorkRequest, PeerWriteWorkRequest,
};
use crate::network::Node;

impl<'scope, 'env> PollingScope<'scope, 'env, Node> {
    /// Posts sends to multiple peers, returning handles for manual polling.
    pub fn post_scatter_send<'wr, I>(
        &mut self,
        wrs: I,
    ) -> TransportResult<Vec<ScopedPendingWork<'scope>>>
    where
        I: IntoIterator<Item = PeerSendWorkRequest<'wr, 'env>>,
        'env: 'wr,
    {
        wrs.into_iter().map(|wr| self.post_send(wr)).collect()
    }

    /// Posts RDMA writes to multiple peers, returning handles for manual polling.
    pub fn post_scatter_write<'wr, I>(
        &mut self,
        wrs: I,
    ) -> TransportResult<Vec<ScopedPendingWork<'scope>>>
    where
        I: IntoIterator<Item = PeerWriteWorkRequest<'wr, 'env>>,
        'env: 'wr,
    {
        wrs.into_iter().map(|wr| self.post_write(wr)).collect()
    }

    /// Posts receives from multiple peers, returning handles for manual polling.
    pub fn post_gather_receive<'wr, I>(
        &mut self,
        wrs: I,
    ) -> TransportResult<Vec<ScopedPendingWork<'scope>>>
    where
        I: IntoIterator<Item = PeerReceiveWorkRequest<'wr, 'env>>,
        'env: 'wr,
    {
        wrs.into_iter().map(|wr| self.post_receive(wr)).collect()
    }

    /// Posts RDMA reads from multiple peers, returning handles for manual polling.
    pub fn post_gather_read<'wr, I>(
        &mut self,
        wrs: I,
    ) -> TransportResult<Vec<ScopedPendingWork<'scope>>>
    where
        I: IntoIterator<Item = PeerReadWorkRequest<'wr, 'env>>,
        'env: 'wr,
    {
        wrs.into_iter().map(|wr| self.post_read(wr)).collect()
    }

    /// Posts the same send to multiple peers, returning handles for manual polling.
    pub fn post_multicast_send<'wr, I>(
        &mut self,
        peers: I,
        wr: SendWorkRequest<'wr, 'env>,
    ) -> TransportResult<Vec<ScopedPendingWork<'scope>>>
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

impl Node {
    /// Posts sends to multiple peers and blocks until all complete.
    pub fn scatter_send<'op, I>(&'op mut self, wrs: I) -> TransportResult<Vec<WorkSuccess>>
    where
        I: IntoIterator<Item = PeerSendWorkRequest<'op, 'op>>,
    {
        self.multi_channel.scatter_send(wrs)
    }

    /// Posts RDMA writes to multiple peers and blocks until all complete.
    pub fn scatter_write<'op, I>(&'op mut self, wrs: I) -> TransportResult<Vec<WorkSuccess>>
    where
        I: IntoIterator<Item = PeerWriteWorkRequest<'op, 'op>>,
    {
        self.multi_channel.scatter_write(wrs)
    }

    /// Posts receives from multiple peers and blocks until all complete.
    pub fn gather_receive<'op, I>(&'op mut self, wrs: I) -> TransportResult<Vec<WorkSuccess>>
    where
        I: IntoIterator<Item = PeerReceiveWorkRequest<'op, 'op>>,
    {
        self.multi_channel.gather_receive(wrs)
    }

    /// Posts RDMA reads from multiple peers and blocks until all complete.
    pub fn gather_read<'op, I>(&'op mut self, wrs: I) -> TransportResult<Vec<WorkSuccess>>
    where
        I: IntoIterator<Item = PeerReadWorkRequest<'op, 'op>>,
    {
        self.multi_channel.gather_read(wrs)
    }

    /// Posts the same send to multiple peers and blocks until all complete.
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
    /// Posts sends to multiple peers without polling for completion.
    ///
    /// # Safety
    /// See [`Channel::send_unpolled`](crate::channel::Channel::send_unpolled).
    pub unsafe fn scatter_send_unpolled<'wr, 'data, I>(
        &mut self,
        wrs: I,
    ) -> IbvResult<Vec<PendingWork<'data>>>
    where
        I: IntoIterator<Item = PeerSendWorkRequest<'wr, 'data>>,
        'data: 'wr,
    {
        unsafe { self.multi_channel.scatter_send_unpolled(wrs) }
    }

    /// Posts RDMA writes to multiple peers without polling for completion.
    ///
    /// # Safety
    /// See [`Channel::write_unpolled`](crate::channel::Channel::write_unpolled).
    pub unsafe fn scatter_write_unpolled<'wr, 'data, I>(
        &mut self,
        wrs: I,
    ) -> IbvResult<Vec<PendingWork<'data>>>
    where
        I: IntoIterator<Item = PeerWriteWorkRequest<'wr, 'data>>,
        'data: 'wr,
    {
        unsafe { self.multi_channel.scatter_write_unpolled(wrs) }
    }

    /// Posts receives from multiple peers without polling for completion.
    ///
    /// # Safety
    /// See [`Channel::receive_unpolled`](crate::channel::Channel::receive_unpolled).
    pub unsafe fn gather_receive_unpolled<'wr, 'data, I>(
        &mut self,
        wrs: I,
    ) -> IbvResult<Vec<PendingWork<'data>>>
    where
        I: IntoIterator<Item = PeerReceiveWorkRequest<'wr, 'data>>,
        'data: 'wr,
    {
        unsafe { self.multi_channel.gather_receive_unpolled(wrs) }
    }

    /// Posts RDMA reads from multiple peers without polling for completion.
    ///
    /// # Safety
    /// See [`Channel::read_unpolled`](crate::channel::Channel::read_unpolled).
    pub unsafe fn gather_read_unpolled<'wr, 'data, I>(
        &mut self,
        wrs: I,
    ) -> IbvResult<Vec<PendingWork<'data>>>
    where
        I: IntoIterator<Item = PeerReadWorkRequest<'wr, 'data>>,
        'data: 'wr,
    {
        unsafe { self.multi_channel.gather_read_unpolled(wrs) }
    }

    /// Posts the same send to multiple peers without polling for completion.
    ///
    /// # Safety
    /// See [`Channel::send_unpolled`](crate::channel::Channel::send_unpolled).
    pub unsafe fn multicast_send_unpolled<'wr, 'data, I>(
        &mut self,
        peers: I,
        wr: SendWorkRequest<'wr, 'data>,
    ) -> IbvResult<Vec<PendingWork<'data>>>
    where
        I: IntoIterator<Item = usize>,
    {
        unsafe { self.multi_channel.multicast_send_unpolled(peers, wr) }
    }
}
