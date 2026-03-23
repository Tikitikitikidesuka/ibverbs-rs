use crate::channel::TransportResult;
use crate::ibverbs::work::{SendWorkRequest, WorkSuccess};
use crate::multi_channel::MultiChannel;
use crate::multi_channel::work_request::*;

impl MultiChannel {
    /// Posts sends to multiple peers and blocks until all complete.
    pub fn scatter_send<'op, I>(&'op mut self, wrs: I) -> TransportResult<Vec<WorkSuccess>>
    where
        I: IntoIterator<Item = PeerSendWorkRequest<'op, 'op>>,
    {
        self.manual_scope(|s| {
            let wrs = s.post_scatter_send(wrs)?;
            wrs.into_iter().map(|wr| wr.spin_poll()).collect()
        })
    }

    /// Posts RDMA writes to multiple peers and blocks until all complete.
    pub fn scatter_write<'op, I>(&'op mut self, wrs: I) -> TransportResult<Vec<WorkSuccess>>
    where
        I: IntoIterator<Item = PeerWriteWorkRequest<'op, 'op>>,
    {
        self.manual_scope(|s| {
            let wrs = s.post_scatter_write(wrs)?;
            wrs.into_iter().map(|wr| wr.spin_poll()).collect()
        })
    }

    /// Posts receives from multiple peers and blocks until all complete.
    pub fn gather_receive<'op, I>(&'op mut self, wrs: I) -> TransportResult<Vec<WorkSuccess>>
    where
        I: IntoIterator<Item = PeerReceiveWorkRequest<'op, 'op>>,
    {
        self.manual_scope(|s| {
            let wrs = s.post_gather_receive(wrs)?;
            wrs.into_iter().map(|wr| wr.spin_poll()).collect()
        })
    }

    /// Posts RDMA reads from multiple peers and blocks until all complete.
    pub fn gather_read<'op, I>(&'op mut self, wrs: I) -> TransportResult<Vec<WorkSuccess>>
    where
        I: IntoIterator<Item = PeerReadWorkRequest<'op, 'op>>,
    {
        self.manual_scope(|s| {
            let wrs = s.post_gather_read(wrs)?;
            wrs.into_iter().map(|wr| wr.spin_poll()).collect()
        })
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
        self.manual_scope(|s| {
            let wrs = s.post_multicast_send(peers, wr)?;
            wrs.into_iter().map(|wr| wr.spin_poll()).collect()
        })
    }
}
