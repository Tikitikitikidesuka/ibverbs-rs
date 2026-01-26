use crate::channel::multi_channel::MultiChannel;
use crate::channel::raw_channel::pending_work::PendingWork;
use crate::ibverbs::scatter_gather_element::{ScatterElement, GatherElement};
use std::io;

impl MultiChannel {
    pub fn scatter_unpolled<'a, I, WR>(
        &'a mut self,
        scatter_sends: I,
    ) -> io::Result<Vec<PendingWork<'a>>>
    where
        I: IntoIterator<Item = (usize, WR)>,
        I::IntoIter: ExactSizeIterator,
        WR: AsRef<[GatherElement<'a>]>,
    {
        scatter_sends
            .into_iter()
            .map(|(peer, sends)| unsafe { self.send_unpolled(peer, sends) })
            .collect()
    }

    pub fn gather_unpolled<'a, I, WR>(
        &'a mut self,
        gather_receives: I,
    ) -> io::Result<Vec<PendingWork<'a>>>
    where
        I: IntoIterator<Item = (usize, WR)>,
        I::IntoIter: ExactSizeIterator,
        WR: AsMut<[ScatterElement<'a>]>,
    {
        gather_receives
            .into_iter()
            .map(|(peer, receives)| unsafe { self.receive_unpolled(peer, receives) })
            .collect()
    }

    pub fn multicast_unpolled<'a, I, WR>(
        &'a mut self,
        sends: WR,
        peers: I,
    ) -> io::Result<Vec<PendingWork<'a>>>
    where
        I: IntoIterator<Item = usize>,
        I::IntoIter: ExactSizeIterator,
        WR: AsRef<[GatherElement<'a>]>,
    {
        peers
            .into_iter()
            .map(|peer| unsafe { self.send_unpolled(peer, sends.as_ref()) })
            .collect()
    }
}
