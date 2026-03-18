use crate::channel::TransportResult;
use crate::channel::pending_work::PendingWork;
use crate::channel::polling_scope::{PollingScope, ScopedPendingWork};
use crate::ibverbs::error::IbvResult;
use crate::ibverbs::work::*;
use crate::multi_channel::MultiChannel;
use crate::multi_channel::work_request::{
    PeerReadWorkRequest, PeerReceiveWorkRequest, PeerSendWorkRequest, PeerWriteWorkRequest,
};

impl<'scope, 'env> PollingScope<'scope, 'env, MultiChannel> {
    pub fn post_send(
        &mut self,
        wr: PeerSendWorkRequest<'_, 'env>,
    ) -> TransportResult<ScopedPendingWork<'scope>> {
        Ok(self.channel_post_send(|m| m.channel(wr.peer), wr.wr)?)
    }

    pub fn post_receive(
        &mut self,
        wr: PeerReceiveWorkRequest<'_, 'env>,
    ) -> TransportResult<ScopedPendingWork<'scope>> {
        Ok(self.channel_post_receive(|m| m.channel(wr.peer), wr.wr)?)
    }

    pub fn post_write(
        &mut self,
        wr: PeerWriteWorkRequest<'_, 'env>,
    ) -> TransportResult<ScopedPendingWork<'scope>> {
        Ok(self.channel_post_write(|m| m.channel(wr.peer), wr.wr)?)
    }

    pub fn post_read(
        &mut self,
        wr: PeerReadWorkRequest<'_, 'env>,
    ) -> TransportResult<ScopedPendingWork<'scope>> {
        Ok(self.channel_post_read(|m| m.channel(wr.peer), wr.wr)?)
    }
}

impl MultiChannel {
    pub fn send<'op>(
        &'op mut self,
        wr: PeerSendWorkRequest<'op, 'op>,
    ) -> TransportResult<WorkSuccess> {
        self.channel(wr.peer)?.send(wr.wr)
    }

    pub fn receive<'op>(
        &'op mut self,
        wr: PeerReceiveWorkRequest<'op, 'op>,
    ) -> TransportResult<WorkSuccess> {
        self.channel(wr.peer)?.receive(wr.wr)
    }

    pub fn write<'op>(
        &'op mut self,
        wr: PeerWriteWorkRequest<'op, 'op>,
    ) -> TransportResult<WorkSuccess> {
        self.channel(wr.peer)?.write(wr.wr)
    }

    pub fn read<'op>(
        &'op mut self,
        wr: PeerReadWorkRequest<'op, 'op>,
    ) -> TransportResult<WorkSuccess> {
        self.channel(wr.peer)?.read(wr.wr)
    }
}

impl MultiChannel {
    pub unsafe fn send_unpolled<'data>(
        &mut self,
        wr: PeerSendWorkRequest<'_, 'data>,
    ) -> IbvResult<PendingWork<'data>> {
        unsafe { self.channel(wr.peer)?.send_unpolled(wr.wr) }
    }

    pub unsafe fn receive_unpolled<'data>(
        &mut self,
        wr: PeerReceiveWorkRequest<'_, 'data>,
    ) -> IbvResult<PendingWork<'data>> {
        unsafe { self.channel(wr.peer)?.receive_unpolled(wr.wr) }
    }

    pub unsafe fn write_unpolled<'data>(
        &mut self,
        wr: PeerWriteWorkRequest<'_, 'data>,
    ) -> IbvResult<PendingWork<'data>> {
        unsafe { self.channel(wr.peer)?.write_unpolled(wr.wr) }
    }

    pub unsafe fn read_unpolled<'data>(
        &mut self,
        wr: PeerReadWorkRequest<'_, 'data>,
    ) -> IbvResult<PendingWork<'data>> {
        unsafe { self.channel(wr.peer)?.read_unpolled(wr.wr) }
    }
}
