//! Work request posting operations for a connected queue pair.
//!
//! This module adds the four core RDMA operations to [`QueuePair`] as `unsafe` methods.
//! All four follow the same pattern: build a typed work request, post it with an
//! application-chosen `wr_id`, and poll the corresponding completion queue for the result.
//!
//! | Method | Operation | Queue |
//! |--------|-----------|-------|
//! | [`post_send`](QueuePair::post_send) | Two-sided Send | Send CQ |
//! | [`post_receive`](QueuePair::post_receive) | Two-sided Receive | Recv CQ |
//! | [`post_write`](QueuePair::post_write) | One-sided RDMA Write | Send CQ |
//! | [`post_read`](QueuePair::post_read) | One-sided RDMA Read | Send CQ |

use crate::ibverbs::error::{IbvError, IbvResult};
use crate::ibverbs::queue_pair::QueuePair;
use crate::ibverbs::work::{
    ReadWorkRequest, ReceiveWorkRequest, SendWorkRequest, WriteWorkRequest,
};
use ibverbs_sys::*;
use std::ptr;

impl QueuePair {
    /// Posts a **Send** request to the Send Queue.
    ///
    /// # Safety
    ///
    /// The buffers referenced by the `gather_elements` must remain **valid** and **immutable**
    /// until the work is finished by the hardware (signaled via the Send CQ).
    ///
    /// You must ensure that the lifetime of the data, which was tied to the
    /// `GatherElement`, is manually extended until completion.
    pub unsafe fn post_send(&mut self, wr: SendWorkRequest, wr_id: u64) -> IbvResult<()> {
        let (opcode, __bindgen_anon_1) = match wr.imm_data {
            None => (ibv_wr_opcode::IBV_WR_SEND, Default::default()),
            Some(imm_data) => (
                ibv_wr_opcode::IBV_WR_SEND_WITH_IMM,
                ibv_send_wr__bindgen_ty_1 {
                    imm_data: imm_data.to_be(),
                },
            ),
        };

        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        // length validated at WorkRequest construction
        let mut wr = ibv_send_wr {
            wr_id,
            next: ptr::null::<ibv_send_wr>() as *mut _,
            sg_list: wr.gather_elements.as_ref().as_ptr() as *mut ibv_sge,
            num_sge: wr.gather_elements.as_ref().len() as i32,
            opcode,
            send_flags: ibv_send_flags::IBV_SEND_SIGNALED.0,
            wr: Default::default(),
            qp_type: Default::default(),
            __bindgen_anon_1,
            __bindgen_anon_2: Default::default(),
        };

        unsafe {
            self.post_send_wr(&mut wr).map_err(|errno| {
                IbvError::from_errno_with_msg(errno, "Failed to post send work request")
            })
        }
    }

    /// Posts a **Receive** request to the Receive Queue.
    ///
    /// # Safety
    ///
    /// The buffers referenced by the `scatter_elements` must remain **valid** and **exclusive**
    /// (mutable) until the work is finished by the hardware (signaled via the Recv CQ).
    ///
    /// Accessing these buffers before completion results in a data race (Undefined Behavior).
    pub unsafe fn post_receive(&mut self, wr: ReceiveWorkRequest, wr_id: u64) -> IbvResult<()> {
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        // length validated at WorkRequest construction
        let mut wr = ibv_recv_wr {
            wr_id,
            next: ptr::null::<ibv_send_wr>() as *mut _,
            sg_list: wr.scatter_elements.as_mut().as_mut_ptr() as *mut ibv_sge,
            num_sge: wr.scatter_elements.as_mut().len() as i32,
        };

        unsafe {
            self.post_receive_wr(&mut wr).map_err(|errno| {
                IbvError::from_errno_with_msg(errno, "Failed to post receive work request")
            })
        }
    }

    /// Posts an **RDMA Write** request.
    ///
    /// # Safety
    ///
    /// The buffers referenced by the `gather_elements` must remain **valid** and **immutable**
    /// until the work is finished by the hardware.
    ///
    /// Additionally, the remote address range must be valid on the remote peer; otherwise,
    /// a Remote Access Error will occur.
    pub unsafe fn post_write(&mut self, wr: WriteWorkRequest, wr_id: u64) -> IbvResult<()> {
        let (opcode, __bindgen_anon_1) = match wr.imm_data {
            None => (ibv_wr_opcode::IBV_WR_RDMA_WRITE, Default::default()),
            Some(imm_data) => (
                ibv_wr_opcode::IBV_WR_RDMA_WRITE_WITH_IMM,
                ibv_send_wr__bindgen_ty_1 {
                    imm_data: imm_data.to_be(),
                },
            ),
        };

        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        // length validated at WorkRequest construction
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

        unsafe {
            self.post_send_wr(&mut wr).map_err(|errno| {
                IbvError::from_errno_with_msg(errno, "Failed to post write work request")
            })
        }
    }

    /// Posts an **RDMA Read** request.
    ///
    /// # Safety
    ///
    /// The buffers referenced by the `scatter_elements` must remain **valid** and **exclusive**
    /// (mutable) until the work is finished by the hardware.
    pub unsafe fn post_read(&mut self, wr: ReadWorkRequest, wr_id: u64) -> IbvResult<()> {
        // length validated at WorkRequest construction
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
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

        unsafe { self.post_send_wr(&mut wr) }.map_err(|errno| {
            IbvError::from_errno_with_msg(errno, "Failed to post read work request")
        })
    }

    #[inline(always)]
    unsafe fn post_send_wr(&mut self, wr: &mut ibv_send_wr) -> Result<(), i32> {
        let mut bad_wr: *mut ibv_send_wr = ptr::null::<ibv_send_wr>() as *mut _;
        let ctx = unsafe { *self.qp }.context;
        let ops = &mut unsafe { *ctx }.ops;
        let errno = unsafe {
            ops.post_send
                .as_mut()
                .expect("post_send function pointer should be set by driver")(
                self.qp,
                wr as *mut _,
                &mut bad_wr as *mut _,
            )
        };
        if errno != 0 { Err(errno) } else { Ok(()) }
    }

    #[inline(always)]
    unsafe fn post_receive_wr(&mut self, wr: &mut ibv_recv_wr) -> Result<(), i32> {
        let mut bad_wr: *mut ibv_recv_wr = ptr::null::<ibv_recv_wr>() as *mut _;
        let ctx = unsafe { *self.qp }.context;
        let ops = &mut unsafe { *ctx }.ops;
        let errno = unsafe {
            ops.post_recv
                .as_mut()
                .expect("post_recv function pointer should be set by driver")(
                self.qp,
                wr as *mut _,
                &mut bad_wr as *mut _,
            )
        };
        if errno != 0 { Err(errno) } else { Ok(()) }
    }
}
