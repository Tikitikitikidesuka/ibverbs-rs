use crate::channel::multi_channel::MultiChannel;
use crate::channel::raw_channel::pending_work::{MultiWorkSpinPollResult, WorkPollError};
use crate::ibverbs::remote_memory_region::RemoteMemorySliceMut;
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use crate::ibverbs::work_request::{ReceiveWorkRequest, SendWorkRequest, WriteWorkRequest};
use crate::ibverbs::work_success::WorkSuccess;
use std::borrow::{Borrow, BorrowMut};
use crate::channel::multi_channel::rank_work_request::RankWriteWorkRequest;

impl MultiChannel {
    pub fn scatter<'a, I, E, WR>(&'a mut self, wrs: I) -> MultiWorkSpinPollResult
    where
        I: IntoIterator<Item = (usize, WR)>,
        E: AsRef<[GatherElement<'a>]>,
        WR: Borrow<SendWorkRequest<'a, E>>,
    {
        let res = self.scope(|s| {
            let wrs = s.post_scatter(wrs)?;
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

    pub fn scatter_write<'a, I, E, R, WR>(&'a mut self, wrs: I) -> MultiWorkSpinPollResult
    where
        I: IntoIterator<Item = WR>,
        E: AsRef<[GatherElement<'a>]>,
        R: BorrowMut<RemoteMemorySliceMut<'a>>,
        WR: BorrowMut<RankWriteWorkRequest<'a, E, R>>,
    {
        let res = self.scope(|s| {
            let wrs = s.post_scatter_write(wrs)?;
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

    pub fn gather<'a, I, E, WR>(&'a mut self, wrs: I) -> MultiWorkSpinPollResult
    where
        I: IntoIterator<Item = (usize, WR)>,
        E: AsMut<[ScatterElement<'a>]>,
        WR: BorrowMut<ReceiveWorkRequest<'a, E>>,
    {
        let res = self.scope(|s| {
            let wrs = s.post_gather(wrs)?;
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

    pub fn multicast<'a, I, E, WR>(&'a mut self, peers: I, wr: WR) -> MultiWorkSpinPollResult
    where
        I: IntoIterator<Item = usize>,
        E: AsRef<[GatherElement<'a>]>,
        WR: Borrow<SendWorkRequest<'a, E>>,
    {
        let res = self.scope(|s| {
            let wrs = s.post_multicast(peers, wr)?;
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
