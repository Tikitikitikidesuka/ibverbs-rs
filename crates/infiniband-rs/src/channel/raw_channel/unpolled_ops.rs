use crate::channel::raw_channel::RawChannel;
use crate::channel::raw_channel::pending_work::PendingWork;
use crate::ibverbs::scatter_gather_element::{ScatterElement, GatherElement};
use std::io;

impl RawChannel {
    /// # Safety
    /// The caller must ensure that the returned `IbvWorkRequest` is
    /// **successfully polled to completion by its drop implementation**
    /// before the end of `'a`.
    ///
    /// In particular, the work request must not be leaked (e.g. via
    /// `mem::forget`), as this would end the borrow without dropping
    /// while the hardware may still access the memory.
    ///
    /// ## Protection example
    ///
    /// ```compile_fail
    /// # use infiniband_rs::connection::connection::IbvConnection;
    /// # let mut conn: IbvConnection = unsafe { std::mem::zeroed() };
    /// let mut mem = [0u8; 1024];
    /// let mr = conn.register_mr("foo_mr", mem.as_mut_ptr(), mem.len()).unwrap();
    /// let receive = mr.prepare_send(&mem[0..4]).unwrap();
    /// let wr = unsafe { conn.send_unpolled(&[receive]) }.unwrap();
    ///
    /// // This mutation of mem will not compile while the borrow is alive in the wr,
    /// // preventing partially modified memory from being sent.
    /// (&mut mem[0..4]).copy_from_slice(&[107, 101, 111, 51]);
    /// ```
    ///
    /// ## Safety violation example
    ///
    /// ```no_run
    /// # use infiniband_rs::connection::connection::Connection;
    /// # let mut conn: Connection = unsafe { std::mem::zeroed() };
    /// let mut mem = [0u8; 1024];
    /// let mr = conn.register_mr("foo_mr", mem.as_mut_ptr(), mem.len()).unwrap();
    /// let receive = mr.prepare_receive(&mut mem[0..4]).unwrap();
    /// let wr = unsafe { conn.receive_unpolled(&[receive]) }.unwrap();
    ///
    /// // The work request can be leaked without running its drop.
    /// // The borrow ends but the NIC may still DMA into the memory.
    /// std::mem::forget(wr);
    ///
    /// // This mutation of mem might occur while the send is partially complete.
    /// // This violates Rust's aliasing rules and constitutes UB.
    /// (&mut mem[0..4]).copy_from_slice(&[107, 101, 111, 51]);
    /// ```
    pub unsafe fn send_unpolled<'a>(
        &mut self,
        sends: impl AsRef<[GatherElement<'a>]>,
    ) -> io::Result<PendingWork<'a>> {
        let wr_id = self.get_and_advance_wr_id();
        unsafe { self.qp.post_send(sends.as_ref(), wr_id)? };
        Ok(unsafe { PendingWork::new(wr_id, self.cq.clone()) })
    }

    /// # Safety
    ///
    /// The caller must ensure that the returned `IbvWorkRequest` is
    /// **successfully polled to completion by its drop implementation**
    /// before the end of `'a`.
    ///
    /// In particular, the work request must not be leaked (e.g. via
    /// `mem::forget`), as this would end the borrow without dropping
    /// while the hardware may still access the memory.
    ///
    /// ## Protection example
    ///
    /// ```compile_fail
    /// # use infiniband_rs::connection::connection::IbvConnection;
    /// # let mut conn: IbvConnection = unsafe { std::mem::zeroed() };
    /// let mut mem = [0u8; 1024];
    /// let mr = conn.register_mr("foo_mr", mem.as_mut_ptr(), mem.len()).unwrap();
    /// let receive = mr.prepare_send(&mem[0..4]).unwrap();
    /// let wr = unsafe { conn.send_with_imm_data_unpolled(&[receive], 33) }.unwrap();
    ///
    /// // This mutation of mem will not compile while the borrow is alive in the wr,
    /// // preventing partially modified memory from being sent.
    /// (&mut mem[0..4]).copy_from_slice(&[107, 101, 111, 51]);
    /// ```
    ///
    /// ## Safety violation example
    ///
    /// ```no_run
    /// # use infiniband_rs::connection::connection::Connection;
    /// # let mut conn: Connection = unsafe { std::mem::zeroed() };
    /// let mut mem = [0u8; 1024];
    /// let mr = conn.register_mr("foo_mr", mem.as_mut_ptr(), mem.len()).unwrap();
    /// let receive = mr.prepare_receive(&mut mem[0..4]).unwrap();
    /// let wr = unsafe { conn.send_with_immediate_unpolled(&[receive], 33) }.unwrap();
    ///
    /// // The work request can be leaked without running its drop.
    /// // The borrow ends but the NIC may still DMA into the memory.
    /// std::mem::forget(wr);
    ///
    /// // This mutation of mem might occur while the send is partially complete.
    /// // This violates Rust's aliasing rules and constitutes UB.
    /// (&mut mem[0..4]).copy_from_slice(&[107, 101, 111, 51]);
    /// ```
    pub unsafe fn send_with_immediate_unpolled<'a>(
        &mut self,
        sends: impl AsRef<[GatherElement<'a>]>,
        imm_data: u32,
    ) -> io::Result<PendingWork<'a>> {
        let wr_id = self.get_and_advance_wr_id();
        unsafe {
            self.qp
                .post_send_with_immediate(sends.as_ref(), imm_data, wr_id)?
        };
        Ok(unsafe { PendingWork::new(wr_id, self.cq.clone()) })
    }

    pub fn send_immediate_unpolled<'a>(&mut self, imm_data: u32) -> io::Result<PendingWork<'a>> {
        let wr_id = self.get_and_advance_wr_id();
        self.qp.post_send_immediate(imm_data, wr_id)?;
        Ok(unsafe { PendingWork::new(wr_id, self.cq.clone()) })
    }

    /// # Safety
    /// The caller must ensure that the returned `IbvWorkRequest` is
    /// **successfully polled to completion by its drop implementation**
    /// before the end of `'a`.
    ///
    /// In particular, the work request must not be leaked (e.g. via
    /// `mem::forget`), as this would end the borrow without dropping
    /// while the hardware may still access the memory.
    ///
    /// ## Protection example
    ///
    /// ```compile_fail
    /// # use infiniband_rs::connection::connection::IbvConnection;
    /// # let mut conn: IbvConnection = unsafe { std::mem::zeroed() };
    /// let mut mem = [0u8; 1024];
    /// let mr = conn.register_mr("foo_mr", mem.as_mut_ptr(), mem.len()).unwrap();
    /// let receive = mr.prepare_receive(&mut mem[0..4]).unwrap();
    /// let wr = unsafe { conn.receive_unpolled(&[receive]) }.unwrap();
    ///
    /// // This read of mem will not compile while the borrow is alive in the wr.
    /// println!("{:?}", &mem[0..4]);
    /// ```
    ///
    /// ## Safety violation example
    ///
    /// ```no_run
    /// # use infiniband_rs::connection::connection::Connection;
    /// # let mut conn: Connection = unsafe { std::mem::zeroed() };
    /// let mut mem = [0u8; 1024];
    /// let mr = conn.register_mr("foo_mr", mem.as_mut_ptr(), mem.len()).unwrap();
    /// let receive = mr.prepare_receive(&mut mem[0..4]).unwrap();
    /// let wr = unsafe { conn.receive_unpolled(&[receive]) }.unwrap();
    ///
    /// // The work request can be leaked without running its drop.
    /// // The borrow ends but the NIC may still DMA into the memory.
    /// std::mem::forget(wr);
    ///
    /// // This read of mem might occur while the receive is partially complete.
    /// // This violates Rust's aliasing rules and constitutes UB.
    /// println!("{:?}", &mem[0..4]);
    /// ```
    pub unsafe fn receive_unpolled<'a>(
        &mut self,
        mut receives: impl AsMut<[ScatterElement<'a>]>,
    ) -> io::Result<PendingWork<'a>> {
        let wr_id = self.get_and_advance_wr_id();
        unsafe { self.qp.post_receive(receives.as_mut(), wr_id)? };
        Ok(unsafe { PendingWork::new(wr_id, self.cq.clone()) })
    }

    pub fn receive_immediate_unpolled<'a>(&mut self) -> io::Result<PendingWork<'a>> {
        let wr_id = self.get_and_advance_wr_id();
        self.qp.post_receive_immediate(wr_id)?;
        Ok(unsafe { PendingWork::new(wr_id, self.cq.clone()) })
    }

    fn get_and_advance_wr_id(&mut self) -> u64 {
        let wr_id = self.next_wr_id;
        self.next_wr_id += 1;
        wr_id
    }
}
