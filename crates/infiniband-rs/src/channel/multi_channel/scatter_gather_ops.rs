use crate::channel::multi_channel::MultiChannel;
use crate::channel::raw_channel::pending_work::{MultiWorkSpinPollResult, WorkPollError};
use crate::channel::raw_channel::polling_scope::ScopedPendingWork;
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use crate::ibverbs::work_success::WorkSuccess;
use std::io;

impl MultiChannel {
    pub fn scatter<'a, I, WR>(&'a mut self, scatter_sends: I) -> MultiWorkSpinPollResult
    where
        I: IntoIterator<Item = (usize, WR)>,
        WR: AsRef<[ScatterElement<'a>]>,
    {
        let res = self.scope(|s| {
            let wrs = scatter_sends
                .into_iter()
                .map(|(peer, send)| s.post_send(peer, send))
                .collect::<io::Result<Vec<ScopedPendingWork>>>()?;
            wrs.into_iter()
                .map(|wr| wr.spin_poll())
                .collect::<Result<Vec<WorkSuccess>, WorkPollError>>()
        })?;
        debug_assert!(
            res.is_ok(),
            "unreachable scope error (all wrs manually polled)"
        );
        Ok(res.unwrap())
    }

    pub fn gather<'a, I, WR>(&'a mut self, gather_receives: I) -> MultiWorkSpinPollResult
    where
        I: IntoIterator<Item = (usize, WR)>,
        WR: AsMut<[GatherElement<'a>]>,
    {
        let res = self.scope(|s| {
            let wrs = gather_receives
                .into_iter()
                .map(|(peer, receive)| s.post_receive(peer, receive))
                .collect::<io::Result<Vec<ScopedPendingWork>>>()?;
            wrs.into_iter()
                .map(|wr| wr.spin_poll())
                .collect::<Result<Vec<WorkSuccess>, WorkPollError>>()
        })?;
        debug_assert!(
            res.is_ok(),
            "unreachable scope error (all wrs manually polled)"
        );
        Ok(res.unwrap())
    }
}
