use crate::channel::TransportResult;
use crate::channel::polling_scope::*;
use crate::ibverbs::error::IbvResult;
use crate::multi_channel::MultiChannel;
use crate::multi_channel::work_request::*;

impl MultiChannel {
    pub fn scope<'env, F, T>(&'env mut self, f: F) -> Result<T, ScopeError>
    where
        F: for<'scope> FnOnce(&mut PollingScope<'scope, 'env, MultiChannel>) -> TransportResult<T>,
    {
        PollingScope::run(self, f)
    }

    pub fn manual_scope<'env, F, T>(&'env mut self, f: F) -> TransportResult<T>
    where
        F: for<'scope> FnOnce(&mut PollingScope<'scope, 'env, MultiChannel>) -> TransportResult<T>,
    {
        PollingScope::run_manual(self, f)
    }
}

impl<'scope, 'env> PollingScope<'scope, 'env, MultiChannel> {
    pub fn post_send(
        &mut self,
        wr: PeerSendWorkRequest<'_, 'env>,
    ) -> IbvResult<ScopedPendingWork<'scope>> {
        self.channel_post_send(|m| m.channel(wr.peer), wr.wr)
    }

    pub fn post_receive(
        &mut self,
        wr: PeerReceiveWorkRequest<'_, 'env>,
    ) -> IbvResult<ScopedPendingWork<'scope>> {
        self.channel_post_receive(|m| m.channel(wr.peer), wr.wr)
    }

    pub fn post_write(
        &mut self,
        wr: PeerWriteWorkRequest<'_, 'env>,
    ) -> IbvResult<ScopedPendingWork<'scope>> {
        self.channel_post_write(|m| m.channel(wr.peer), wr.wr)
    }

    pub fn post_read(
        &mut self,
        wr: PeerReadWorkRequest<'_, 'env>,
    ) -> IbvResult<ScopedPendingWork<'scope>> {
        self.channel_post_read(|m| m.channel(wr.peer), wr.wr)
    }
}
