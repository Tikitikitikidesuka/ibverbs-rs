use crate::channel::multi_channel::MultiChannel;
use crate::channel::multi_channel::work_request::{
    PeerReadWorkRequest, PeerReceiveWorkRequest, PeerSendWorkRequest, PeerWriteWorkRequest,
};
use crate::channel::raw_channel::pending_work::PendingWork;
use std::io;

impl MultiChannel {
    pub unsafe fn send_unpolled<'data>(
        &mut self,
        wr: PeerSendWorkRequest<'_, 'data>,
    ) -> io::Result<PendingWork<'data>> {
        unsafe { self.channel(wr.peer)?.send_unpolled(wr.wr) }
    }

    pub unsafe fn receive_unpolled<'data>(
        &mut self,
        wr: PeerReceiveWorkRequest<'_, 'data>,
    ) -> io::Result<PendingWork<'data>> {
        unsafe { self.channel(wr.peer)?.receive_unpolled(wr.wr) }
    }

    pub unsafe fn write_unpolled<'data>(
        &mut self,
        wr: PeerWriteWorkRequest<'_, 'data>,
    ) -> io::Result<PendingWork<'data>> {
        unsafe { self.channel(wr.peer)?.write_unpolled(wr.wr) }
    }

    pub unsafe fn read_unpolled<'data>(
        &mut self,
        wr: PeerReadWorkRequest<'_, 'data>,
    ) -> io::Result<PendingWork<'data>> {
        unsafe { self.channel(wr.peer)?.read_unpolled(wr.wr) }
    }
}
