use crate::channel::pending_work::{
    MultiWorkPollError, MultiWorkSpinPollResult, PendingWork, WorkSpinPollResult,
};
use crate::channel::polling_scope::PollingScope;
use crate::ibverbs::work_request::SendWorkRequest;
use crate::multi_channel::MultiChannel;
use crate::multi_channel::work_request::{
    PeerReadWorkRequest, PeerReceiveWorkRequest, PeerSendWorkRequest, PeerWriteWorkRequest,
};
use crate::network::Node;
use std::io;

impl Node {
    pub fn scope<'env, F, R>(&'env mut self, f: F) -> Result<R, MultiWorkPollError>
    where
        F: for<'scope> FnOnce(&mut PollingScope<'scope, 'env, MultiChannel>) -> R,
    {
        PollingScope::run(&mut self.multi_channel, f)
    }
}

impl Node {
    pub fn send<'op>(&'op mut self, wr: PeerSendWorkRequest<'op, 'op>) -> WorkSpinPollResult {
        self.multi_channel.send(wr)
    }

    pub fn receive<'op>(&'op mut self, wr: PeerReceiveWorkRequest<'op, 'op>) -> WorkSpinPollResult {
        self.multi_channel.receive(wr)
    }

    pub fn write<'op>(&'op mut self, wr: PeerWriteWorkRequest<'op, 'op>) -> WorkSpinPollResult {
        self.multi_channel.write(wr)
    }

    pub fn read<'op>(&'op mut self, wr: PeerReadWorkRequest<'op, 'op>) -> WorkSpinPollResult {
        self.multi_channel.read(wr)
    }
}

impl Node {
    pub unsafe fn send_unpolled<'data>(
        &mut self,
        wr: PeerSendWorkRequest<'_, 'data>,
    ) -> io::Result<PendingWork<'data>> {
        unsafe { self.multi_channel.send_unpolled(wr) }
    }

    pub unsafe fn receive_unpolled<'data>(
        &mut self,
        wr: PeerReceiveWorkRequest<'_, 'data>,
    ) -> io::Result<PendingWork<'data>> {
        unsafe { self.multi_channel.receive_unpolled(wr) }
    }

    pub unsafe fn write_unpolled<'data>(
        &mut self,
        wr: PeerWriteWorkRequest<'_, 'data>,
    ) -> io::Result<PendingWork<'data>> {
        unsafe { self.multi_channel.write_unpolled(wr) }
    }

    pub unsafe fn read_unpolled<'data>(
        &mut self,
        wr: PeerReadWorkRequest<'_, 'data>,
    ) -> io::Result<PendingWork<'data>> {
        unsafe { self.multi_channel.read_unpolled(wr) }
    }
}

impl Node {
    pub fn scatter_send<'op, I>(&'op mut self, wrs: I) -> MultiWorkSpinPollResult
    where
        I: IntoIterator<Item = PeerSendWorkRequest<'op, 'op>>,
    {
        self.multi_channel.scatter_send(wrs)
    }

    pub fn scatter_write<'op, I>(&'op mut self, wrs: I) -> MultiWorkSpinPollResult
    where
        I: IntoIterator<Item = PeerWriteWorkRequest<'op, 'op>>,
    {
        self.multi_channel.scatter_write(wrs)
    }

    pub fn gather_receive<'op, I>(&'op mut self, wrs: I) -> MultiWorkSpinPollResult
    where
        I: IntoIterator<Item = PeerReceiveWorkRequest<'op, 'op>>,
    {
        self.multi_channel.gather_receive(wrs)
    }

    pub fn gather_read<'op, I>(&'op mut self, wrs: I) -> MultiWorkSpinPollResult
    where
        I: IntoIterator<Item = PeerReadWorkRequest<'op, 'op>>,
    {
        self.multi_channel.gather_read(wrs)
    }

    pub fn multicast_send<'op, I>(
        &'op mut self,
        peers: I,
        wr: SendWorkRequest<'op, 'op>,
    ) -> MultiWorkSpinPollResult
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
    ) -> io::Result<Vec<PendingWork<'data>>>
    where
        I: IntoIterator<Item = PeerSendWorkRequest<'wr, 'data>>,
        'data: 'wr,
    {
        self.multi_channel.scatter_send_unpolled(wrs)
    }

    pub fn scatter_write_unpolled<'wr, 'data, I>(
        &mut self,
        wrs: I,
    ) -> io::Result<Vec<PendingWork<'data>>>
    where
        I: IntoIterator<Item = PeerWriteWorkRequest<'wr, 'data>>,
        'data: 'wr,
    {
        self.multi_channel.scatter_write_unpolled(wrs)
    }

    pub fn gather_receive_unpolled<'wr, 'data, I>(
        &mut self,
        wrs: I,
    ) -> io::Result<Vec<PendingWork<'data>>>
    where
        I: IntoIterator<Item = PeerReceiveWorkRequest<'wr, 'data>>,
        'data: 'wr,
    {
        self.multi_channel.gather_receive_unpolled(wrs)
    }

    pub fn gather_read_unpolled<'wr, 'data, I>(
        &mut self,
        wrs: I,
    ) -> io::Result<Vec<PendingWork<'data>>>
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
    ) -> io::Result<Vec<PendingWork<'data>>>
    where
        I: IntoIterator<Item = usize>,
    {
        self.multi_channel.multicast_send_unpolled(peers, wr)
    }
}
