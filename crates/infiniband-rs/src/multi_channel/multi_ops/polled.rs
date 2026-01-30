use crate::channel::pending_work::{MultiWorkSpinPollResult, WorkPollError};
use crate::ibverbs::work_request::{SendWorkRequest};
use crate::ibverbs::work_success::WorkSuccess;
use crate::multi_channel::MultiChannel;
use crate::multi_channel::work_request::{
    PeerReadWorkRequest, PeerReceiveWorkRequest, PeerSendWorkRequest, PeerWriteWorkRequest,
};

impl MultiChannel {
    pub fn scatter_send<'op, I>(&'op mut self, wrs: I) -> MultiWorkSpinPollResult
    where
        I: IntoIterator<Item = PeerSendWorkRequest<'op, 'op>>,
    {
        let res = self.scope(|s| {
            let wrs = s.post_scatter_send(wrs)?;
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

    pub fn scatter_write<'op, I>(&'op mut self, wrs: I) -> MultiWorkSpinPollResult
    where
        I: IntoIterator<Item = PeerWriteWorkRequest<'op, 'op>>,
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

    pub fn gather_receive<'op, I>(&'op mut self, wrs: I) -> MultiWorkSpinPollResult
    where
        I: IntoIterator<Item = PeerReceiveWorkRequest<'op, 'op>>,
    {
        let res = self.scope(|s| {
            let wrs = s.post_gather_receive(wrs)?;
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

    pub fn gather_read<'op, I>(&'op mut self, wrs: I) -> MultiWorkSpinPollResult
    where
        I: IntoIterator<Item = PeerReadWorkRequest<'op, 'op>>,
    {
        let res = self.scope(|s| {
            let wrs = s.post_gather_read(wrs)?;
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

    pub fn multicast_send<'op, I>(
        &'op mut self,
        peers: I,
        wr: SendWorkRequest<'op, 'op>,
    ) -> MultiWorkSpinPollResult
    where
        I: IntoIterator<Item = usize>,
    {
        let res = self.scope(|s| {
            let wrs = s.post_multicast_send(peers, wr)?;
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
