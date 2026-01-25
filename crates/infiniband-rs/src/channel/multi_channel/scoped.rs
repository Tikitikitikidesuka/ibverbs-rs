use crate::channel::multi_channel::MultiChannel;
use crate::channel::raw_channel::pending_work::MultiWorkPollError;
use crate::channel::raw_channel::polling_scope::{PollingScope, ScopedPendingWork};
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use std::io;

impl MultiChannel {
    pub fn scope<'env, F, R>(&'env mut self, f: F) -> Result<R, MultiWorkPollError>
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

    pub fn post_scatter<I, WR>(
        &mut self,
        scatter_sends: I,
    ) -> io::Result<Vec<ScopedPendingWork<'scope>>>
    where
        I: IntoIterator<Item = (usize, WR)>,
        WR: AsRef<[ScatterElement<'env>]>,
    {
        scatter_sends
            .into_iter()
            .map(|(peer, sends)| self.channel_post_send(|m| m.channel(peer), sends))
            .collect()
    }

    pub fn post_gather<I, WR>(
        &mut self,
        gather_receives: I,
    ) -> io::Result<Vec<ScopedPendingWork<'scope>>>
    where
        I: IntoIterator<Item = (usize, WR)>,
        WR: AsMut<[GatherElement<'env>]>,
    {
        gather_receives
            .into_iter()
            .map(|(peer, sends)| self.channel_post_receive(|m| m.channel(peer), sends))
            .collect()
    }
}
