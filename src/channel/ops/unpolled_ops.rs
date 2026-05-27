use crate::channel::Channel;
use crate::channel::pending_work::PendingWork;
use crate::ibverbs::error::IbvResult;
use crate::ibverbs::queue_pair::ops::WorkRequest;

impl Channel {
    /// Posts a send operation without polling for completion.
    ///
    /// # Safety
    /// The returned [`PendingWork`] must not be leaked (e.g. via [`mem::forget`](std::mem::forget)),
    /// as this would end the borrow without dropping while the hardware may still access the memory.
    /// Prefer [`scope`](Channel::scope) or [`manual_scope`](Channel::manual_scope) which prevent this.
    pub unsafe fn post<'data, W>(&mut self, wr: W) -> IbvResult<PendingWork<'data>>
    where
        W: WorkRequest,
    {
        let wr_id = self.get_and_advance_wr_id();
        unsafe { self.qp.post(wr, wr_id)? };
        Ok(unsafe { PendingWork::new(wr_id, self.cq.clone()) })
    }

    fn get_and_advance_wr_id(&mut self) -> u64 {
        let wr_id = self.next_wr_id;
        self.next_wr_id += 1;
        wr_id
    }
}
