use crate::ibverbs::protection_domain::IbvProtectionDomainInner;
use ibverbs_sys::*;
use std::fmt::Debug;
use std::sync::Arc;
use std::{io, ptr};

pub struct IbvQueuePair {
    pub(super) pd: Arc<IbvProtectionDomainInner>,
    pub(super) qp: *mut ibv_qp,
}

unsafe impl Send for IbvQueuePair {}
unsafe impl Sync for IbvQueuePair {}

impl Drop for IbvQueuePair {
    fn drop(&mut self) {
        let qp = self.qp;
        let errno = unsafe { ibv_destroy_qp(self.qp) };
        if errno != 0 {
            let debug_text = format!("{:?}", self);
            let e = io::Error::from_raw_os_error(errno);
            log::error!(
                "({debug_text}) -> Failed to release completion queue with `ibv_destroy_qp({qp:p})`: {e}"
            );
        }
    }
}

impl Debug for IbvQueuePair {
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

impl IbvQueuePair {
    pub unsafe fn post_send(&mut self, local: &[ibv_sge], wr_id: u64) -> io::Result<()> {
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

    pub unsafe fn post_send_with_imm(
        &mut self,
        local: &[ibv_sge],
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
    pub unsafe fn post_send_wr(&mut self, wr: &mut ibv_send_wr) -> io::Result<()> {
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

    pub unsafe fn post_receive(&mut self, local: &[ibv_sge], wr_id: u64) -> io::Result<()> {
        let mut wr = ibv_recv_wr {
            wr_id,
            next: ptr::null::<ibv_send_wr>() as *mut _,
            sg_list: local.as_ptr() as *mut ibv_sge,
            num_sge: local.len() as i32,
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
