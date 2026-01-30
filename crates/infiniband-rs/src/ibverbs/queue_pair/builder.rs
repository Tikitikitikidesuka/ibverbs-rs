use crate::ibverbs::completion_queue::CompletionQueue;
use crate::ibverbs::prepared_queue_pair::PreparedQueuePair;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::ibverbs::queue_pair::QueuePair;
use crate::ibverbs::queue_pair::config::{
    AccessFlags, AckTimeout, MaxAckRetries, MaxRnrRetries, MinRnrTimer, PacketSequenceNumber,
};
use bon::bon;
use ibverbs_sys::{
    ibv_access_flags, ibv_create_qp, ibv_qp_cap, ibv_qp_init_attr, ibv_qp_type, ibv_srq,
};
use std::ffi::c_void;
use std::{io, ptr};

#[bon]
impl QueuePair {
    /// # Errors
    ///  - `EINVAL`: Invalid `ProtectionDomain`, sending or receiving `Context`, or invalid value
    ///    provided in `max_send_wr`, `max_recv_wr`, or in `max_inline_data`.
    ///  - `ENOMEM`: Not enough resources to complete this operation.
    ///  - `ENOSYS`: QP with this Transport Service Type isn't supported by this RDMA device.
    ///  - `EPERM`: Not enough permissions to create a QP with this Transport Service Type.
    #[builder]
    pub fn builder(
        pd: &ProtectionDomain,
        send_cq: &CompletionQueue,
        recv_cq: &CompletionQueue,
        access: AccessFlags,
        #[builder(default = 16)] max_send_wr: u32,
        #[builder(default = 16)] max_recv_wr: u32,
        #[builder(default = 16)] max_send_sge: u32,
        #[builder(default = 16)] max_recv_sge: u32,
        #[builder(default)] max_rnr_retries: MaxRnrRetries,
        #[builder(default)] max_ack_retries: MaxAckRetries,
        #[builder(default)] min_rnr_timer: MinRnrTimer,
        #[builder(default)] ack_timeout: AckTimeout,
        #[builder(default)] pnr: PacketSequenceNumber,
    ) -> io::Result<PreparedQueuePair> {
        let mut attr = ibv_qp_init_attr {
            qp_context: ptr::null::<c_void>() as *mut _,
            send_cq: send_cq.inner.cq as *const _ as *mut _,
            recv_cq: recv_cq.inner.cq as *const _ as *mut _,
            srq: ptr::null::<ibv_srq>() as *mut _,
            cap: ibv_qp_cap {
                max_send_wr,
                max_recv_wr,
                max_send_sge,
                max_recv_sge,
                max_inline_data: 0,
            },
            qp_type: ibv_qp_type::IBV_QPT_RC,
            sq_sig_all: 0,
        };

        let qp = unsafe { ibv_create_qp(pd.inner.pd, &mut attr as *mut _) };
        if qp.is_null() {
            Err(io::Error::last_os_error())
        } else {
            log::debug!("IbvQueuePair created");
            Ok(PreparedQueuePair {
                qp: QueuePair {
                    pd: pd.clone(),
                    send_cq: send_cq.clone(),
                    recv_cq: recv_cq.clone(),
                    qp,
                },
                lid: pd.inner.context.inner.query_port()?.lid,

                max_send_wr,
                max_send_sge,
                max_recv_wr,
                max_recv_sge,
                access: ibv_access_flags(access.code()),
                max_rnr_retries: max_rnr_retries.code(),
                max_ack_retries: max_ack_retries.code(),
                min_rnr_timer: min_rnr_timer.code(),
                ack_timeout: ack_timeout.code(),
            })
        }
    }
}
