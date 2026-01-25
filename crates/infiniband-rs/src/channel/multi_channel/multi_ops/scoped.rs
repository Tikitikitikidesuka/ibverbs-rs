use crate::channel::multi_channel::MultiChannel;
use crate::channel::raw_channel::polling_scope::{PollingScope, ScopedPendingWork};
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use std::io;

impl<'scope, 'env> PollingScope<'scope, 'env, MultiChannel> {
    pub fn post_scatter<I, WR>(
        &mut self,
        scatter_sends: I,
    ) -> io::Result<Vec<ScopedPendingWork<'scope>>>
    where
        I: IntoIterator<Item = (usize, WR)>,
        I::IntoIter: ExactSizeIterator,
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
        I::IntoIter: ExactSizeIterator,
        WR: AsMut<[GatherElement<'env>]>,
    {
        gather_receives
            .into_iter()
            .map(|(peer, sends)| self.channel_post_receive(|m| m.channel(peer), sends))
            .collect()
    }

    pub fn post_multicast<I, WR>(
        &mut self,
        sends: WR,
        peers: I,
    ) -> io::Result<Vec<ScopedPendingWork<'scope>>>
    where
        I: IntoIterator<Item = usize>,
        I::IntoIter: ExactSizeIterator,
        WR: AsRef<[ScatterElement<'env>]>,
    {
        peers
            .into_iter()
            .map(|peer| self.post_send(peer, sends.as_ref()))
            .collect::<io::Result<Vec<_>>>()
    }
}
