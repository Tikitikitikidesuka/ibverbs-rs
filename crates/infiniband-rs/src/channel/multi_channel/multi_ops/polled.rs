use crate::channel::multi_channel::MultiChannel;
use crate::channel::raw_channel::pending_work::{MultiWorkSpinPollResult, WorkPollError};
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use crate::ibverbs::work_success::WorkSuccess;

impl MultiChannel {
    pub fn scatter<'a, I, WR>(&'a mut self, scatter_sends: I) -> MultiWorkSpinPollResult
    where
        I: IntoIterator<Item = (usize, WR)>,
        I::IntoIter: ExactSizeIterator,
        WR: AsRef<[ScatterElement<'a>]>,
    {
        let res = self.scope(|s| {
            let wrs = s.post_scatter(scatter_sends)?;
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
        I::IntoIter: ExactSizeIterator,
        WR: AsMut<[GatherElement<'a>]>,
    {
        let res = self.scope(|s| {
            let wrs = s.post_gather(gather_receives)?;
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

    pub fn multicast<'a, I, WR>(&'a mut self, sends: WR, peers: I) -> MultiWorkSpinPollResult
    where
        I: IntoIterator<Item = usize>,
        I::IntoIter: ExactSizeIterator,
        WR: AsRef<[ScatterElement<'a>]>,
    {
        let res = self.scope(|s| {
            let wrs = s.post_multicast(sends, peers)?;
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
