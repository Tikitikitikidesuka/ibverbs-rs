use crate::channel::TransportResult;
use crate::channel::{PollingScope, ScopedPendingWork};
use crate::ibverbs::work::SendWorkRequest;
use crate::multi_channel::MultiChannel;
use crate::multi_channel::work_request::*;

impl<'scope, 'env> PollingScope<'scope, 'env, MultiChannel> {
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
