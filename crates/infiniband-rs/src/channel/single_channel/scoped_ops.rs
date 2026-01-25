use crate::channel::raw_channel::polling_scope::{
    PollingScope, ScopedPendingWork,
};
use crate::channel::single_channel::SingleChannel;
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use std::io;
use crate::channel::raw_channel::pending_work::MultiWorkPollError;

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
        sends: impl AsRef<[ScatterElement<'env>]>,
    ) -> io::Result<ScopedPendingWork<'scope>> {
        self.channel_post_send(|s| Ok(&mut s.channel), sends)
    }

    pub fn post_send_with_immediate(
        &mut self,
        sends: impl AsRef<[ScatterElement<'env>]>,
        imm_data: u32,
    ) -> io::Result<ScopedPendingWork<'scope>> {
        self.channel_post_send_with_immediate(|s| Ok(&mut s.channel), sends, imm_data)
    }

    pub fn post_receive(
        &mut self,
        receives: impl AsMut<[GatherElement<'env>]>,
    ) -> io::Result<ScopedPendingWork<'scope>> {
        self.channel_post_receive(|s| Ok(&mut s.channel), receives)
    }
}
