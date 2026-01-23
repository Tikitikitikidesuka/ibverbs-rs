use crate::channel::polling_scope::{PollingScope, PollingScopeError, ScopedPendingWork};
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use crate::multi_channel::MultiChannel;
use std::io;

impl MultiChannel {
    pub fn scope<'env, F, R>(&'env mut self, f: F) -> Result<R, PollingScopeError>
    where
        F: for<'scope> FnOnce(&mut PollingScope<'scope, 'env, MultiChannel>) -> R,
    {
        PollingScope::run(self, f)
    }
}

impl<'scope, 'env> PollingScope<'scope, 'env, MultiChannel> {
    pub fn post_send(
        &mut self,
        peer: usize,
        sends: impl AsRef<[ScatterElement<'env>]>,
    ) -> io::Result<ScopedPendingWork<'scope>> {
        self.channel_post_send(|m| m.channel(peer), sends)
    }

    pub fn post_send_with_immediate(
        &mut self,
        peer: usize,
        sends: impl AsRef<[ScatterElement<'env>]>,
        imm_data: u32,
    ) -> io::Result<ScopedPendingWork<'scope>> {
        self.channel_post_send_with_immediate(|m| m.channel(peer), sends, imm_data)
    }

    pub fn post_receive(
        &mut self,
        peer: usize,
        receives: impl AsMut<[GatherElement<'env>]>,
    ) -> io::Result<ScopedPendingWork<'scope>> {
        self.channel_post_receive(|m| m.channel(peer), receives)
    }
}
