use crate::channel::raw_channel::RawChannel;
use crate::channel::raw_channel::pending_work::MultiWorkPollError;
use crate::channel::raw_channel::polling_scope::{PollingScope, ScopedPendingWork};
use crate::ibverbs::work_request::{
    ReadWorkRequest, ReceiveWorkRequest, SendWorkRequest, WriteWorkRequest,
};
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
    pub fn post_send(
        &mut self,
        wr: SendWorkRequest<'_, 'env>,
    ) -> io::Result<ScopedPendingWork<'scope>> {
        self.channel_post_send(|s| Ok(s), wr)
    }

    pub fn post_receive(
        &mut self,
        wr: ReceiveWorkRequest<'_, 'env>,
    ) -> io::Result<ScopedPendingWork<'scope>> {
        self.channel_post_receive(|s| Ok(s), wr)
    }

    pub fn post_write(
        &mut self,
        wr: WriteWorkRequest<'_, 'env>,
    ) -> io::Result<ScopedPendingWork<'scope>> {
        self.channel_post_write(|s| Ok(s), wr)
    }

    pub fn post_read(
        &mut self,
        wr: ReadWorkRequest<'_, 'env>,
    ) -> io::Result<ScopedPendingWork<'scope>> {
        self.channel_post_read(|s| Ok(s), wr)
    }
}
