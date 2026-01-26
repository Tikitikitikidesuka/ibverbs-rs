use crate::channel::multi_channel::MultiChannel;
use crate::channel::raw_channel::pending_work::{MultiWorkSpinPollResult, WorkPollError};
use crate::ibverbs::scatter_gather_element::{ScatterElement, GatherElement};
use crate::ibverbs::work_success::WorkSuccess;

impl MultiChannel {
    pub fn scatter<'a, I, WR>(&'a mut self, scatter_sends: I) -> MultiWorkSpinPollResult
    where
        I: IntoIterator<Item = (usize, WR)>,
        WR: AsRef<[GatherElement<'a>]>,
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

    pub fn scatter_with_immediate<'a, I, WR>(
        &'a mut self,
        scatter_sends: I,
    ) -> MultiWorkSpinPollResult
    where
        I: IntoIterator<Item = (usize, WR, u32)>,
        WR: AsRef<[GatherElement<'a>]>,
    {
        let res = self.scope(|s| {
            let wrs = s.post_scatter_with_immediate(scatter_sends)?;
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

    pub fn scatter_immediate<I>(&mut self, scatter_sends: I) -> MultiWorkSpinPollResult
    where
        I: IntoIterator<Item = (usize, u32)>,
    {
        let res = self.scope(|s| {
            let wrs = s.post_scatter_immediate(scatter_sends)?;
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
        WR: AsMut<[ScatterElement<'a>]>,
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

    pub fn gather_immediate<I>(&mut self, peers: I) -> MultiWorkSpinPollResult
    where
        I: IntoIterator<Item = usize>,
    {
        let res = self.scope(|s| {
            let wrs = s.post_gather_immediate(peers)?;
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

    pub fn multicast<'a, I, WR>(&'a mut self, peers: I, sends: WR) -> MultiWorkSpinPollResult
    where
        I: IntoIterator<Item = usize>,
        WR: AsRef<[GatherElement<'a>]>,
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

    pub fn multicast_with_immediate<'a, I, WR>(
        &'a mut self,
        peers: I,
        sends: WR,
        imm_data: u32,
    ) -> MultiWorkSpinPollResult
    where
        I: IntoIterator<Item = usize>,
        WR: AsRef<[GatherElement<'a>]>,
    {
        let res = self.scope(|s| {
            let wrs = s.post_multicast_with_immediate(peers, sends, imm_data)?;
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

    pub fn multicast_immediate<I>(&mut self, peers: I, imm_data: u32) -> MultiWorkSpinPollResult
    where
        I: IntoIterator<Item = usize>,
    {
        let res = self.scope(|s| {
            let wrs = s.post_multicast_immediate(peers, imm_data)?;
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
