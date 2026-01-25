use crate::ibverbs::protection_domain::ProtectionDomainInner;
use ibverbs_sys::*;
use std::fmt::Debug;
use std::sync::Arc;
use std::{io, ptr};
use crate::ibverbs::completion_queue::CompletionQueueInner;
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};

pub struct QueuePair {
    pub(super) pd: Arc<ProtectionDomainInner>,
    pub(super) send_cq: Arc<CompletionQueueInner>,
    pub(super) recv_cq: Arc<CompletionQueueInner>,
    pub(super) qp: *mut ibv_qp,
}

unsafe impl Send for QueuePair {}
unsafe impl Sync for QueuePair {}

impl Drop for QueuePair {
    fn drop(&mut self) {
        log::debug!("IbvQueuePair destroyed");
        let qp = self.qp;
        let errno = unsafe { ibv_destroy_qp(self.qp) };
        if errno != 0 {
            let debug_text = format!("{:?}", self);
            let e = io::Error::from_raw_os_error(errno);
            log::error!(
                "({debug_text}) -> Failed to destroy queue pair with `ibv_destroy_qp({qp:p})`: {e}"
            );
        }
    }
}

impl Debug for QueuePair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.debug_struct("IbvQueuePair")
            .field("handle", &unsafe { (*self.qp).handle })
            .field("qp_num", &unsafe { (*self.qp).qp_num })
            .field("state", &unsafe { (*self.qp).state })
            .field("type", &unsafe { (*self.qp).qp_type })
            .field("send_cq_handle", &unsafe { (*(*self.qp).send_cq).handle })
            .field("recv_cq_handle", &unsafe { (*(*self.qp).recv_cq).handle })
            .field("pd", &self.pd)
            .finish()
    }
}

impl QueuePair {
    /// # Safety
    /// The buffers pointed to by GatherElement must remain valid until the work request issued
    /// is complete. That is, the buffers pointed to by the gather elements must live for at least 'a.
    pub unsafe fn post_send<'a>(&mut self, local: &[ScatterElement<'a>], wr_id: u64) -> io::Result<()> {
        let mut wr = ibv_send_wr {
            wr_id,
            next: ptr::null::<ibv_send_wr>() as *mut _,
            sg_list: local.as_ptr() as *mut ibv_sge,
            num_sge: local.len() as i32,
            opcode: ibv_wr_opcode::IBV_WR_SEND,
            send_flags: ibv_send_flags::IBV_SEND_SIGNALED.0,
            wr: Default::default(),
            qp_type: Default::default(),
            __bindgen_anon_1: Default::default(),
            __bindgen_anon_2: Default::default(),
        };

        unsafe { self.post_send_wr(&mut wr) }
    }

    /// # Safety
    /// The buffers pointed to by GatherElement must remain valid until the work request issued
    /// is complete. That is, the buffers pointed to by the gather elements must live for at least 'a.
    pub unsafe fn post_send_with_immediate<'a>(
        &mut self,
        local: &[ScatterElement<'a>],
        imm_data: u32,
        wr_id: u64,
    ) -> io::Result<()> {
        let mut wr = ibv_send_wr {
            wr_id,
            next: ptr::null::<ibv_send_wr>() as *mut _,
            sg_list: local.as_ptr() as *mut ibv_sge,
            num_sge: local.len() as i32,
            opcode: ibv_wr_opcode::IBV_WR_SEND_WITH_IMM,
            send_flags: ibv_send_flags::IBV_SEND_SIGNALED.0,
            wr: Default::default(),
            qp_type: Default::default(),
            __bindgen_anon_1: ibv_send_wr__bindgen_ty_1 { imm_data },
            __bindgen_anon_2: Default::default(),
        };

        unsafe { self.post_send_wr(&mut wr) }
    }

    #[inline(always)]
    unsafe fn post_send_wr(&mut self, wr: &mut ibv_send_wr) -> io::Result<()> {
        let mut bad_wr: *mut ibv_send_wr = ptr::null::<ibv_send_wr>() as *mut _;
        let ctx = unsafe { *self.qp }.context;
        let ops = &mut unsafe { *ctx }.ops;
        let errno = unsafe {
            ops.post_send.as_mut().unwrap()(self.qp, wr as *mut _, &mut bad_wr as *mut _)
        };
        if errno != 0 {
            Err(io::Error::from_raw_os_error(errno))
        } else {
            Ok(())
        }
    }

    /// # Safety
    /// The buffers pointed to by GatherElement must remain valid until the work request issued
    /// is complete. That is, the buffers pointed to by the gather elements must live for at least 'a.
    pub unsafe fn post_receive<'a>(&mut self, local: &mut [GatherElement<'a>], wr_id: u64) -> io::Result<()> {
        let mut wr = ibv_recv_wr {
            wr_id,
            next: ptr::null::<ibv_send_wr>() as *mut _,
            sg_list: local.as_mut_ptr() as *mut ibv_sge,
            num_sge: local.len() as i32, // todo: fix possible error on overflow
        };

        let mut bad_wr: *mut ibv_recv_wr = ptr::null::<ibv_recv_wr>() as *mut _;
        let ctx = unsafe { *self.qp }.context;
        let ops = &mut unsafe { *ctx }.ops;
        let errno = unsafe {
            ops.post_recv.as_mut().unwrap()(self.qp, &mut wr as *mut _, &mut bad_wr as *mut _)
        };
        if errno != 0 {
            Err(io::Error::from_raw_os_error(errno))
        } else {
            Ok(())
        }
    }
}
