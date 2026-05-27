use crate::channel::polling_scope::*;
use crate::channel::{Channel, TransportResult};
use crate::ibverbs::queue_pair::ops::WorkRequest;

impl<'scope, 'env> PollingScope<'scope, 'env, Channel> {
    pub fn post<W>(&mut self, wr: W) -> TransportResult<ScopedPendingWork<'scope>>
    where
        W: WorkRequest,
    {
        Ok(self.channel_post(|s| Ok(s), wr)?)
    }
}
