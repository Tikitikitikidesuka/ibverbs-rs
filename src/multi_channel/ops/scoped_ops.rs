use crate::channel::TransportResult;
use crate::channel::polling_scope::{PollingScope, ScopedPendingWork};
use crate::ibverbs::work::SendWorkRequest;
use crate::multi_channel::MultiChannel;
use crate::multi_channel::work_request::*;

impl<'scope, 'env> PollingScope<'scope, 'env, MultiChannel> {
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
