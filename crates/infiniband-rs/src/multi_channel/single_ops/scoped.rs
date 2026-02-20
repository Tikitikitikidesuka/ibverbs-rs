use crate::channel::TransportResult;
use crate::channel::polling_scope::*;
use crate::multi_channel::MultiChannel;
use crate::multi_channel::work_request::*;

impl MultiChannel {
    pub fn scope<'env, F, T, E>(&'env mut self, f: F) -> Result<T, ScopeError<E>>
    where
        F: for<'scope> FnOnce(&mut PollingScope<'scope, 'env, MultiChannel>) -> Result<T, E>,
    {
        PollingScope::run(self, f)
    }

    pub fn manual_scope<'env, F, T, E>(&'env mut self, f: F) -> Result<T, E>
    where
        F: for<'scope> FnOnce(&mut PollingScope<'scope, 'env, MultiChannel>) -> Result<T, E>,
    {
        PollingScope::run_manual(self, f)
    }
}

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
