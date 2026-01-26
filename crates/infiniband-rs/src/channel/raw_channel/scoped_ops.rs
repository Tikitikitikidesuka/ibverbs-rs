use crate::channel::raw_channel::RawChannel;
use crate::channel::raw_channel::pending_work::MultiWorkPollError;
use crate::channel::raw_channel::polling_scope::{PollingScope, ScopedPendingWork};
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use crate::ibverbs::work_request::{ReceiveWorkRequest, SendWorkRequest};
use std::io;

impl RawChannel {
    pub fn scope<'env, F, R>(&'env mut self, f: F) -> Result<R, MultiWorkPollError>
    where
        F: for<'scope> FnOnce(&mut PollingScope<'scope, 'env, RawChannel>) -> R,
    {
        PollingScope::run(self, f)
    }
}

impl<'scope, 'env> PollingScope<'scope, 'env, RawChannel> {
    pub fn post_send<E: AsRef<[GatherElement<'env>]>>(
        &mut self,
        wr: SendWorkRequest<'env, E>,
    ) -> io::Result<ScopedPendingWork<'scope>> {
        self.channel_post_send(|s| Ok(s), wr)
    }

    pub fn post_receive<E: AsMut<[ScatterElement<'env>]>>(
        &mut self,
        wr: ReceiveWorkRequest<'env, E>,
    ) -> io::Result<ScopedPendingWork<'scope>> {
        self.channel_post_receive(|s| Ok(s), wr)
    }

    /*
    pub fn post_write(
        &mut self,
        wr: &mut WriteWorkRequest<'_, 'env>,
    ) -> io::Result<ScopedPendingWork<'scope>> {
        self.channel_post_write(|s| Ok(s), wr)
    }

    pub fn post_read(
        &mut self,
        wr: &mut ReadWorkRequest<'_, 'env>,
    ) -> io::Result<ScopedPendingWork<'scope>> {
        self.channel_post_read(|s| Ok(s), wr)
    }
    */
}
