use crate::channel::polling_scope::*;
use crate::channel::{Channel, TransportResult};
use crate::ibverbs::work::{
    ReadWorkRequest, ReceiveWorkRequest, SendWorkRequest, WriteWorkRequest,
};

impl<'scope, 'env> PollingScope<'scope, 'env, Channel> {
    pub fn post_send(
        &mut self,
        wr: SendWorkRequest<'_, 'env>,
    ) -> TransportResult<ScopedPendingWork<'scope>> {
        Ok(self.channel_post_send(|s| Ok(s), wr)?)
    }

    pub fn post_receive(
        &mut self,
        wr: ReceiveWorkRequest<'_, 'env>,
    ) -> TransportResult<ScopedPendingWork<'scope>> {
        Ok(self.channel_post_receive(|s| Ok(s), wr)?)
    }

    pub fn post_write(
        &mut self,
        wr: WriteWorkRequest<'_, 'env>,
    ) -> TransportResult<ScopedPendingWork<'scope>> {
        Ok(self.channel_post_write(|s| Ok(s), wr)?)
    }

    pub fn post_read(
        &mut self,
        wr: ReadWorkRequest<'_, 'env>,
    ) -> TransportResult<ScopedPendingWork<'scope>> {
        Ok(self.channel_post_read(|s| Ok(s), wr)?)
    }
}
