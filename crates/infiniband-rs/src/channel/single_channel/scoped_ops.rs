use crate::channel::raw_channel::pending_work::MultiWorkPollError;
use crate::channel::raw_channel::polling_scope::{PollingScope, ScopedPendingWork};
use crate::channel::single_channel::SingleChannel;
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use crate::ibverbs::work_request::{
    ReadWorkRequest, ReceiveWorkRequest, SendWorkRequest, WriteWorkRequest,
};
use std::borrow::{Borrow, BorrowMut};
use std::io;

impl SingleChannel {
    pub fn scope<'env, F, R>(&'env mut self, f: F) -> Result<R, MultiWorkPollError>
    where
        F: for<'scope> FnOnce(&mut PollingScope<'scope, 'env, SingleChannel>) -> R,
    {
        PollingScope::run(self, f)
    }
}

impl<'scope, 'env> PollingScope<'scope, 'env, SingleChannel> {
    pub fn post_send(
        &mut self,
        wr: SendWorkRequest<'_, 'env>,
    ) -> io::Result<ScopedPendingWork<'scope>> {
        self.channel_post_send(|s| Ok(&mut s.channel), wr)
    }

    pub fn post_receive(
        &mut self,
        wr: ReceiveWorkRequest<'_, 'env>,
    ) -> io::Result<ScopedPendingWork<'scope>> {
        self.channel_post_receive(|s| Ok(&mut s.channel), wr)
    }

    pub fn post_write(
        &mut self,
        wr: WriteWorkRequest<'_, 'env>,
    ) -> io::Result<ScopedPendingWork<'scope>> {
        self.channel_post_write(|s| Ok(&mut s.channel), wr)
    }

    pub fn post_read(
        &mut self,
        wr: ReadWorkRequest<'_, 'env>,
    ) -> io::Result<ScopedPendingWork<'scope>> {
        self.channel_post_read(|s| Ok(&mut s.channel), wr)
    }
}
