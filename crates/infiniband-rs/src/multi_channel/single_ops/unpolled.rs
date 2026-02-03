use crate::channel::pending_work::PendingWork;
use crate::ibverbs::error::IbvResult;
use crate::multi_channel::MultiChannel;
use crate::multi_channel::work_request::*;

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
