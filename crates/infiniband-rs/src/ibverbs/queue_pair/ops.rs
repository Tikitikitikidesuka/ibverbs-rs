use crate::ibverbs::queue_pair::QueuePair;
use crate::ibverbs::work_request::*;
use ibverbs_sys::*;
use std::{io, ptr};

impl QueuePair {
    /// # Safety
    /// The buffers pointed to by the work request in its gather elements must remain
    /// valid and cannot be mutated until the work is finished by the hardware.
    /// They must also not be aliased by a mutable reference until then.
    pub unsafe fn post_send(&mut self, wr: SendWorkRequest, wr_id: u64) -> io::Result<()> {
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
    /// The buffers pointed to by the work request in its scatter elements must remain
    /// valid and cannot be read or mutated until the work is finished by the hardware.
    /// They must not be aliased by shared or mutable references until then.
    pub unsafe fn post_receive(
        &mut self,
        wr: ReceiveWorkRequest,
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

    /// It is important to notice how remote memory regions work. todo: explain
    /// The `RemoteMemoryRegion`'s length attribute is only a marker to respect bounds locally.
    /// If a `RemoteMemoryRegion` is created with length n but gather regions with an added length
    /// of m greater than n is RDMA written into it, the serialized
    /// # Safety
    /// The buffers pointed to by the work request in its gather elements must remain
    /// valid and cannot be read or mutated until the work is finished by the hardware.
    /// They must not be aliased by shared or mutable references until then.
    pub unsafe fn post_write(&mut self, wr: WriteWorkRequest, wr_id: u64) -> io::Result<()> {
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
                    remote_addr: wr.remote_mr.address(),
                    rkey: wr.remote_mr.rkey(),
                },
            },
            qp_type: Default::default(),
            __bindgen_anon_1,
            __bindgen_anon_2: Default::default(),
        };

        unsafe { self.post_send_wr(&mut wr) }
    }

    /// # Safety
    /// The buffers pointed to by the work request in its gather elements must remain
    /// valid and cannot be read or mutated until the work is finished by the hardware.
    /// They must not be aliased by shared or mutable references until then.
    pub unsafe fn post_read(&mut self, wr: ReadWorkRequest, wr_id: u64) -> io::Result<()> {
        let mut wr = ibv_send_wr {
            wr_id,
            next: ptr::null::<ibv_send_wr>() as *mut _,
            sg_list: wr.scatter_elements.as_ptr() as *mut ibv_sge,
            num_sge: wr.scatter_elements.len() as i32,
            opcode: ibv_wr_opcode::IBV_WR_RDMA_READ,
            send_flags: ibv_send_flags::IBV_SEND_SIGNALED.0,
            wr: ibv_send_wr__bindgen_ty_2 {
                rdma: ibv_send_wr__bindgen_ty_2__bindgen_ty_1 {
                    remote_addr: wr.remote_mr.address(),
                    rkey: wr.remote_mr.rkey(),
                },
            },
            qp_type: Default::default(),
            __bindgen_anon_1: Default::default(),
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
