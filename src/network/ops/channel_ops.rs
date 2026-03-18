use crate::channel::TransportResult;
use crate::channel::pending_work::PendingWork;
use crate::channel::polling_scope::{PollingScope, ScopedPendingWork};
use crate::ibverbs::error::IbvResult;
use crate::ibverbs::work::WorkSuccess;
use crate::multi_channel::work_request::{
    PeerReadWorkRequest, PeerReceiveWorkRequest, PeerSendWorkRequest, PeerWriteWorkRequest,
};
use crate::network::Node;

impl<'scope, 'env> PollingScope<'scope, 'env, Node> {
    pub fn post_send(
        &mut self,
        wr: PeerSendWorkRequest<'_, 'env>,
    ) -> TransportResult<ScopedPendingWork<'scope>> {
        Ok(self.channel_post_send(|n| n.multi_channel.channel(wr.peer), wr.wr)?)
    }

    pub fn post_receive(
        &mut self,
        wr: PeerReceiveWorkRequest<'_, 'env>,
    ) -> TransportResult<ScopedPendingWork<'scope>> {
        Ok(self.channel_post_receive(|n| n.multi_channel.channel(wr.peer), wr.wr)?)
    }

    pub fn post_write(
        &mut self,
        wr: PeerWriteWorkRequest<'_, 'env>,
    ) -> TransportResult<ScopedPendingWork<'scope>> {
        Ok(self.channel_post_write(|n| n.multi_channel.channel(wr.peer), wr.wr)?)
    }

    pub fn post_read(
        &mut self,
        wr: PeerReadWorkRequest<'_, 'env>,
    ) -> TransportResult<ScopedPendingWork<'scope>> {
        Ok(self.channel_post_read(|n| n.multi_channel.channel(wr.peer), wr.wr)?)
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
