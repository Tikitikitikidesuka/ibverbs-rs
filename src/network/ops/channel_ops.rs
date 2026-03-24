use crate::channel::TransportResult;
use crate::channel::{PendingWork, PollingScope, ScopedPendingWork};
use crate::ibverbs::error::IbvResult;
use crate::ibverbs::work::WorkSuccess;
use crate::multi_channel::{
    PeerReadWorkRequest, PeerReceiveWorkRequest, PeerSendWorkRequest, PeerWriteWorkRequest,
};
use crate::network::Node;

impl<'scope, 'env> PollingScope<'scope, 'env, Node> {
    /// Posts a send to the work request's target peer, returning a handle for manual polling.
    pub fn post_send(
        &mut self,
        wr: PeerSendWorkRequest<'_, 'env>,
    ) -> TransportResult<ScopedPendingWork<'scope>> {
        Ok(self.channel_post_send(|n| n.multi_channel.channel(wr.peer), wr.wr)?)
    }

    /// Posts a receive to the work request's target peer, returning a handle for manual polling.
    pub fn post_receive(
        &mut self,
        wr: PeerReceiveWorkRequest<'_, 'env>,
    ) -> TransportResult<ScopedPendingWork<'scope>> {
        Ok(self.channel_post_receive(|n| n.multi_channel.channel(wr.peer), wr.wr)?)
    }

    /// Posts an RDMA write to the work request's target peer, returning a handle for manual polling.
    pub fn post_write(
        &mut self,
        wr: PeerWriteWorkRequest<'_, 'env>,
    ) -> TransportResult<ScopedPendingWork<'scope>> {
        Ok(self.channel_post_write(|n| n.multi_channel.channel(wr.peer), wr.wr)?)
    }

    /// Posts an RDMA read to the work request's target peer, returning a handle for manual polling.
    pub fn post_read(
        &mut self,
        wr: PeerReadWorkRequest<'_, 'env>,
    ) -> TransportResult<ScopedPendingWork<'scope>> {
        Ok(self.channel_post_read(|n| n.multi_channel.channel(wr.peer), wr.wr)?)
    }
}

impl Node {
    /// Posts a send to the work request's target peer and blocks until it completes.
    pub fn send<'op>(
        &'op mut self,
        wr: PeerSendWorkRequest<'op, 'op>,
    ) -> TransportResult<WorkSuccess> {
        self.multi_channel.send(wr)
    }

    /// Posts a receive to the work request's target peer and blocks until it completes.
    pub fn receive<'op>(
        &'op mut self,
        wr: PeerReceiveWorkRequest<'op, 'op>,
    ) -> TransportResult<WorkSuccess> {
        self.multi_channel.receive(wr)
    }

    /// Posts an RDMA write to the work request's target peer and blocks until it completes.
    pub fn write<'op>(
        &'op mut self,
        wr: PeerWriteWorkRequest<'op, 'op>,
    ) -> TransportResult<WorkSuccess> {
        self.multi_channel.write(wr)
    }

    /// Posts an RDMA read to the work request's target peer and blocks until it completes.
    pub fn read<'op>(
        &'op mut self,
        wr: PeerReadWorkRequest<'op, 'op>,
    ) -> TransportResult<WorkSuccess> {
        self.multi_channel.read(wr)
    }
}

impl Node {
    /// Posts a send to the target peer without polling for completion.
    ///
    /// # Safety
    /// See [`Channel::send_unpolled`](crate::channel::Channel::send_unpolled).
    pub unsafe fn send_unpolled<'data>(
        &mut self,
        wr: PeerSendWorkRequest<'_, 'data>,
    ) -> IbvResult<PendingWork<'data>> {
        unsafe { self.multi_channel.send_unpolled(wr) }
    }

    /// Posts a receive to the target peer without polling for completion.
    ///
    /// # Safety
    /// See [`Channel::receive_unpolled`](crate::channel::Channel::receive_unpolled).
    pub unsafe fn receive_unpolled<'data>(
        &mut self,
        wr: PeerReceiveWorkRequest<'_, 'data>,
    ) -> IbvResult<PendingWork<'data>> {
        unsafe { self.multi_channel.receive_unpolled(wr) }
    }

    /// Posts an RDMA write to the target peer without polling for completion.
    ///
    /// # Safety
    /// See [`Channel::write_unpolled`](crate::channel::Channel::write_unpolled).
    pub unsafe fn write_unpolled<'data>(
        &mut self,
        wr: PeerWriteWorkRequest<'_, 'data>,
    ) -> IbvResult<PendingWork<'data>> {
        unsafe { self.multi_channel.write_unpolled(wr) }
    }

    /// Posts an RDMA read to the target peer without polling for completion.
    ///
    /// # Safety
    /// See [`Channel::read_unpolled`](crate::channel::Channel::read_unpolled).
    pub unsafe fn read_unpolled<'data>(
        &mut self,
        wr: PeerReadWorkRequest<'_, 'data>,
    ) -> IbvResult<PendingWork<'data>> {
        unsafe { self.multi_channel.read_unpolled(wr) }
    }
}
