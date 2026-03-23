use crate::channel::Channel;
use crate::channel::pending_work::PendingWork;
use crate::ibverbs::error::IbvResult;
use crate::ibverbs::work::{
    ReadWorkRequest, ReceiveWorkRequest, SendWorkRequest, WriteWorkRequest,
};

impl Channel {
    /// Posts a send operation without polling for completion.
    ///
    /// # Safety
    /// The returned [`PendingWork`] must not be leaked (e.g. via [`mem::forget`](std::mem::forget)),
    /// as this would end the borrow without dropping while the hardware may still access the memory.
    /// Prefer [`scope`](Channel::scope) or [`manual_scope`](Channel::manual_scope) which prevent this.
    pub unsafe fn send_unpolled<'data>(
        &mut self,
        wr: SendWorkRequest<'_, 'data>,
    ) -> IbvResult<PendingWork<'data>> {
        let wr_id = self.get_and_advance_wr_id();
        unsafe { self.qp.post_send(wr, wr_id)? };
        Ok(unsafe { PendingWork::new(wr_id, self.cq.clone()) })
    }

    /// Posts a receive operation without polling for completion.
    ///
    /// # Safety
    /// The returned [`PendingWork`] must not be leaked (e.g. via [`mem::forget`](std::mem::forget)),
    /// as this would end the borrow without dropping while the hardware may still access the memory.
    /// Prefer [`scope`](Channel::scope) or [`manual_scope`](Channel::manual_scope) which prevent this.
    pub unsafe fn receive_unpolled<'data>(
        &mut self,
        wr: ReceiveWorkRequest<'_, 'data>,
    ) -> IbvResult<PendingWork<'data>> {
        let wr_id = self.get_and_advance_wr_id();
        unsafe { self.qp.post_receive(wr, wr_id)? };
        Ok(unsafe { PendingWork::new(wr_id, self.cq.clone()) })
    }

    /// Posts an RDMA write operation without polling for completion.
    ///
    /// # Safety
    /// The returned [`PendingWork`] must not be leaked (e.g. via [`mem::forget`](std::mem::forget)),
    /// as this would end the borrow without dropping while the hardware may still access the memory.
    /// Prefer [`scope`](Channel::scope) or [`manual_scope`](Channel::manual_scope) which prevent this.
    pub unsafe fn write_unpolled<'data>(
        &mut self,
        wr: WriteWorkRequest<'_, 'data>,
    ) -> IbvResult<PendingWork<'data>> {
        let wr_id = self.get_and_advance_wr_id();
        unsafe { self.qp.post_write(wr, wr_id)? };
        Ok(unsafe { PendingWork::new(wr_id, self.cq.clone()) })
    }

    /// Posts an RDMA read operation without polling for completion.
    ///
    /// # Safety
    /// The returned [`PendingWork`] must not be leaked (e.g. via [`mem::forget`](std::mem::forget)),
    /// as this would end the borrow without dropping while the hardware may still access the memory.
    /// Prefer [`scope`](Channel::scope) or [`manual_scope`](Channel::manual_scope) which prevent this.
    pub unsafe fn read_unpolled<'data>(
        &mut self,
        wr: ReadWorkRequest<'_, 'data>,
    ) -> IbvResult<PendingWork<'data>> {
        let wr_id = self.get_and_advance_wr_id();
        unsafe { self.qp.post_read(wr, wr_id)? };
        Ok(unsafe { PendingWork::new(wr_id, self.cq.clone()) })
    }

    fn get_and_advance_wr_id(&mut self) -> u64 {
        let wr_id = self.next_wr_id;
        self.next_wr_id += 1;
        wr_id
    }
}
