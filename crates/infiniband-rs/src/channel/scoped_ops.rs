use crate::channel::polling_scope::*;
use crate::channel::{Channel, TransportResult};
use crate::ibverbs::error::IbvResult;
use crate::ibverbs::work_request::*;

impl Channel {
    pub fn scope<'env, F, T>(&'env mut self, f: F) -> Result<T, ScopeError>
    where
        F: for<'scope> FnOnce(&mut PollingScope<'scope, 'env, Channel>) -> TransportResult<T>,
    {
        PollingScope::run(self, f)
    }

    pub fn manual_scope<'env, F, T>(&'env mut self, f: F) -> TransportResult<T>
    where
        F: for<'scope> FnOnce(&mut PollingScope<'scope, 'env, Channel>) -> TransportResult<T>,
    {
        PollingScope::run_manual(self, f)
    }
}

impl<'scope, 'env> PollingScope<'scope, 'env, Channel> {
    pub fn post_send(
        &mut self,
        wr: SendWorkRequest<'_, 'env>,
    ) -> IbvResult<ScopedPendingWork<'scope>> {
        self.channel_post_send(|s| Ok(s), wr)
    }

    pub fn post_receive(
        &mut self,
        wr: ReceiveWorkRequest<'_, 'env>,
    ) -> IbvResult<ScopedPendingWork<'scope>> {
        self.channel_post_receive(|s| Ok(s), wr)
    }

    pub fn post_write(
        &mut self,
        wr: WriteWorkRequest<'_, 'env>,
    ) -> IbvResult<ScopedPendingWork<'scope>> {
        self.channel_post_write(|s| Ok(s), wr)
    }

    pub fn post_read(
        &mut self,
        wr: ReadWorkRequest<'_, 'env>,
    ) -> IbvResult<ScopedPendingWork<'scope>> {
        self.channel_post_read(|s| Ok(s), wr)
    }
}
