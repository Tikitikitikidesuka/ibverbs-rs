use crate::ibverbs::completion_queue::CompletionQueueInner;
use crate::ibverbs::protection_domain::ProtectionDomainInner;
use crate::ibverbs::remote_memory_region::{
    RemoteMemoryRegion, RemoteMemorySlice, RemoteMemorySliceMut,
};
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use crate::ibverbs::work_request::{ReceiveWorkRequest, SendWorkRequest};
use ibverbs_sys::*;
use std::fmt::Debug;
use std::sync::Arc;
use std::{io, ptr};

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
    pub unsafe fn post_send<'a, E: AsRef<[GatherElement<'a>]>>(
        &mut self,
        wr: SendWorkRequest<'a, E>,
        wr_id: u64,
    ) -> io::Result<()> {
        let (opcode, __bindgen_anon_1) = match wr.imm_data {
            None => (ibv_wr_opcode::IBV_WR_SEND, Default::default()),
            Some(imm_data) => (
                ibv_wr_opcode::IBV_WR_SEND_WITH_IMM,
                ibv_send_wr__bindgen_ty_1 {
                    imm_data: imm_data.to_be(),
                },
            ),
        };

        let mut wr = ibv_send_wr {
            wr_id,
            next: ptr::null::<ibv_send_wr>() as *mut _,
            sg_list: wr.gather_elements.as_ref().as_ptr() as *mut ibv_sge,
            num_sge: wr.gather_elements.as_ref().len() as i32, // todo: fix possible error on overflow
            opcode,
            send_flags: ibv_send_flags::IBV_SEND_SIGNALED.0,
            wr: Default::default(),
            qp_type: Default::default(),
            __bindgen_anon_1,
            __bindgen_anon_2: Default::default(),
        };

        unsafe { self.post_send_wr(&mut wr) }
    }

    /// # Safety
    /// The buffers pointed to by GatherElement must remain valid until the work request issued
    /// is complete. That is, the buffers pointed to by the gather elements must live for at least 'a.
    pub unsafe fn post_receive<'a, E: AsMut<[ScatterElement<'a>]>>(
        &mut self,
        mut wr: ReceiveWorkRequest<'a, E>,
        wr_id: u64,
    ) -> io::Result<()> {
        let mut wr = ibv_recv_wr {
            wr_id,
            next: ptr::null::<ibv_send_wr>() as *mut _,
            sg_list: wr.scatter_elements.as_mut().as_mut_ptr() as *mut ibv_sge,
            num_sge: wr.scatter_elements.as_mut().len() as i32, // todo: fix possible error on overflow
        };

        unsafe { self.post_receive_wr(&mut wr) }
    }

    /*
    /// The buffers pointed to by ScatterElement must remain valid until the work request issued
    /// is complete. That is, the buffers pointed to by the gather elements must live for at least 'a.
    pub unsafe fn post_write<'a>(
        &mut self,
        wr: &mut WriteWorkRequest<'_, 'a>,
        wr_id: u64,
    ) -> io::Result<()> {
        let (opcode, __bindgen_anon_1) = match wr.imm_data {
            None => (ibv_wr_opcode::IBV_WR_RDMA_WRITE, Default::default()),
            Some(imm_data) => (
                ibv_wr_opcode::IBV_WR_RDMA_WRITE_WITH_IMM,
                ibv_send_wr__bindgen_ty_1 {
                    imm_data: imm_data.to_be(),
                },
            ),
        };

        let mut wr = ibv_send_wr {
            wr_id,
            next: ptr::null::<ibv_send_wr>() as *mut _,
            sg_list: wr.gather_elements.as_ptr() as *mut ibv_sge,
            num_sge: wr.gather_elements.len() as i32,
            opcode,
            send_flags: ibv_send_flags::IBV_SEND_SIGNALED.0,
            wr: ibv_send_wr__bindgen_ty_2 {
                rdma: ibv_send_wr__bindgen_ty_2__bindgen_ty_1 {
                    remote_addr: wr.remote_slice.addr as u64,
                    rkey: wr.remote_slice.rkey,
                },
            },
            qp_type: Default::default(),
            __bindgen_anon_1,
            __bindgen_anon_2: Default::default(),
        };

        unsafe { self.post_send_wr(&mut wr) }
    }

    pub unsafe fn post_read<'a>(
        &mut self,
        wr: &mut ReadWorkRequest<'_, 'a>,
        wr_id: u64,
    ) -> io::Result<()> {
        let mut wr = ibv_send_wr {
            wr_id,
            next: ptr::null::<ibv_send_wr>() as *mut _,
            sg_list: wr.scatter_elements.as_ptr() as *mut ibv_sge,
            num_sge: wr.scatter_elements.len() as i32,
            opcode: ibv_wr_opcode::IBV_WR_RDMA_READ,
            send_flags: ibv_send_flags::IBV_SEND_SIGNALED.0,
            wr: ibv_send_wr__bindgen_ty_2 {
                rdma: ibv_send_wr__bindgen_ty_2__bindgen_ty_1 {
                    remote_addr: wr.remote_slice.addr as u64,
                    rkey: wr.remote_slice.rkey,
                },
            },
            qp_type: Default::default(),
            __bindgen_anon_1: Default::default(),
            __bindgen_anon_2: Default::default(),
        };

        unsafe { self.post_send_wr(&mut wr) }
    }
    */

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

    #[inline(always)]
    unsafe fn post_receive_wr(&mut self, wr: &mut ibv_recv_wr) -> io::Result<()> {
        let mut bad_wr: *mut ibv_recv_wr = ptr::null::<ibv_recv_wr>() as *mut _;
        let ctx = unsafe { *self.qp }.context;
        let ops = &mut unsafe { *ctx }.ops;
        let errno = unsafe {
            ops.post_recv.as_mut().unwrap()(self.qp, wr as *mut _, &mut bad_wr as *mut _)
        };
        if errno != 0 {
            Err(io::Error::from_raw_os_error(errno))
        } else {
            Ok(())
        }
    }
}
