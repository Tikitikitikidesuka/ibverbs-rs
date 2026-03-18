use crate::channel::TransportResult;
use crate::channel::pending_work::PendingWork;
use crate::channel::polling_scope::{PollingScope, ScopedPendingWork};
use crate::ibverbs::error::IbvResult;
use crate::ibverbs::work::{SendWorkRequest, WorkSuccess};
use crate::multi_channel::work_request::{
    PeerReadWorkRequest, PeerReceiveWorkRequest, PeerSendWorkRequest, PeerWriteWorkRequest,
};
use crate::network::Node;

impl<'scope, 'env> PollingScope<'scope, 'env, Node> {
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
