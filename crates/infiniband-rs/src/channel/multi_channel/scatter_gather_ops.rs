use crate::channel::multi_channel::MultiChannel;
use crate::channel::raw_channel::pending_work::{MultiWorkSpinPollResult, WorkPollError};
use crate::channel::raw_channel::polling_scope::ScopedPendingWork;
use crate::ibverbs::scatter_gather_element::ScatterElement;
use crate::ibverbs::work_success::WorkSuccess;
use std::io;

/*
impl MultiChannel {
    pub fn scatter<'a, I, WR>(&mut self, peer: usize, sends: I) -> MultiWorkSpinPollResult
    where
        I: IntoIterator<Item = (usize, WR)>,
        WR: AsRef<[ScatterElement<'a>]>,
    {
        let xd = self.scope(|s| {
            let wrs = sends
                .into_iter()
                .map(|(peer, send)| s.post_send(peer, send.as_ref()))
                .collect::<io::Result<Vec<ScopedPendingWork>>>()?;
            let results = wrs
                .into_iter()
                .map(|wr| wr.spin_poll())
                .collect::<Result<Vec<WorkSuccess>, WorkPollError>>()?;
            Ok::<(), WorkPollError>(())
        })??;
        todo!()
    }
}


 */