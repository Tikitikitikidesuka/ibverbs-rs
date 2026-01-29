use crate::channel::multi_channel::MultiChannel;
use crate::channel::multi_channel::work_request::PeerWriteWorkRequest;
use crate::channel::raw_channel::pending_work::PendingWork;
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use crate::ibverbs::work_request::{ReceiveWorkRequest, SendWorkRequest, WriteWorkRequest};
use std::borrow::{Borrow, BorrowMut};
use std::io;

impl MultiChannel {
    pub fn scatter_unpolled<'a, I, E, WR>(&'a mut self, wrs: I) -> io::Result<Vec<PendingWork<'a>>>
    where
        I: IntoIterator<Item = (usize, WR)>,
        E: AsRef<[GatherElement<'a>]>,
        WR: Borrow<SendWorkRequest<'a, E>>,
    {
        wrs.into_iter()
            .map(|(peer, wr)| unsafe { self.send_unpolled(peer, wr) })
            .collect()
    }

    pub fn scatter_write_unpolled<'wr, 'data, I, WR>(
        &'wr mut self,
        wrs: I,
    ) -> io::Result<Vec<PendingWork<'data>>>
    where
        I: IntoIterator<Item = PeerWriteWorkRequest<'wr, 'data>>,
        'data: 'wr,
    {
        wrs.into_iter()
            .map(|wr| unsafe { self.write_unpolled(wr) })
            .collect()
    }

    pub fn gather_unpolled<'a, I, E, WR>(&'a mut self, wrs: I) -> io::Result<Vec<PendingWork<'a>>>
    where
        I: IntoIterator<Item = (usize, WR)>,
        E: AsMut<[ScatterElement<'a>]>,
        WR: BorrowMut<ReceiveWorkRequest<'a, E>>,
    {
        wrs.into_iter()
            .map(|(peer, wr)| unsafe { self.receive_unpolled(peer, wr) })
            .collect()
    }

    pub fn multicast_unpolled<'a, I, E, WR>(
        &'a mut self,
        peers: I,
        wr: WR,
    ) -> io::Result<Vec<PendingWork<'a>>>
    where
        I: IntoIterator<Item = usize>,
        E: AsRef<[GatherElement<'a>]>,
        WR: Borrow<SendWorkRequest<'a, E>>,
    {
        let wr = wr.borrow();
        peers
            .into_iter()
            .map(|peer| unsafe { self.send_unpolled(peer, wr) })
            .collect()
    }
}
