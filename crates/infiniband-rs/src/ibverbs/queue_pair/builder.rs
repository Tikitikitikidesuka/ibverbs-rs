use crate::ibverbs::access_config::AccessFlags;
use crate::ibverbs::completion_queue::CompletionQueue;
use crate::ibverbs::context::IB_PORT;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::ibverbs::queue_pair::QueuePair;
use crate::ibverbs::queue_pair::config::*;
use bon::bon;
use ibverbs_sys::*;
use serde::{Deserialize, Serialize};
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
    #[builder(state_mod(vis = "pub(crate)"))]
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
        #[builder(default)] mtu: MaximumTransferUnit,
        #[builder(default)] send_psn: PacketSequenceNumber,
        #[builder(default)] recv_psn: PacketSequenceNumber,
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
        let endpoint = QueuePairEndpoint {
            num: unsafe { *qp }.qp_num,
            lid: pd.inner.context.inner.query_port()?.lid,
        };
        if qp.is_null() {
            Err(io::Error::last_os_error())
        } else {
            log::debug!("IbvQueuePair created");
            Ok(PreparedQueuePair {
                qp: QueuePair {
                    pd: pd.clone(),
                    _send_cq: send_cq.clone(),
                    _recv_cq: recv_cq.clone(),
                    qp,
                },
                endpoint,

                access,
                max_send_wr,
                max_recv_wr,
                max_send_sge,
                max_recv_sge,
                max_rnr_retries,
                max_ack_retries,
                min_rnr_timer,
                ack_timeout,
                mtu,
                send_psn,
                recv_psn,
            })
        }
    }
}

/// An allocated but uninitialized `QueuePair`.
///
/// Specifically, this `QueuePair` has been allocated with `ibv_create_qp`, but has not yet been
/// initialized with calls to `ibv_modify_qp`.
///
/// To complete the construction of the `QueuePair`, you will need to obtain the
/// `QueuePairEndpoint` of the remote end (by using `PreparedQueuePair::endpoint`), and then call
/// `PreparedQueuePair::handshake` on both sides with the other side's `QueuePairEndpoint`:
#[derive(Debug)]
pub struct PreparedQueuePair {
    qp: QueuePair,
    endpoint: QueuePairEndpoint,

    access: AccessFlags,
    max_send_wr: u32,
    max_recv_wr: u32,
    max_send_sge: u32,
    max_recv_sge: u32,
    max_rnr_retries: MaxRnrRetries,
    max_ack_retries: MaxAckRetries,
    min_rnr_timer: MinRnrTimer,
    ack_timeout: AckTimeout,
    mtu: MaximumTransferUnit,
    send_psn: PacketSequenceNumber,
    recv_psn: PacketSequenceNumber,
}

/// An identifier for the network endpoint of a `QueuePair`.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct QueuePairEndpoint {
    pub num: u32,
    pub lid: u16,
}

impl PreparedQueuePair {
    /// Get the network endpoint for this `QueuePair`.
    ///
    /// This endpoint will need to be communicated to the `QueuePair` on the remote end.
    pub fn endpoint(&self) -> QueuePairEndpoint {
        self.endpoint
    }

    /// Set up the `QueuePair` such that it is ready to exchange packets with a remote `QueuePair`.
    ///
    /// Internally, this uses `ibv_modify_qp` to mark the `QueuePair` as initialized
    /// (`IBV_QPS_INIT`), ready to receive (`IBV_QPS_RTR`), and ready to send (`IBV_QPS_RTS`).
    ///
    /// # Errors
    ///
    ///  - `EINVAL`: Invalid value provided in `attr` or in `attr_mask`.
    ///  - `ENOMEM`: Not enough resources to complete this operation.
    pub fn handshake(self, remote: QueuePairEndpoint) -> io::Result<QueuePair> {
        // Initialize queue pair
        let mut attr = ibv_qp_attr {
            qp_state: ibv_qp_state::IBV_QPS_INIT,
            pkey_index: 0,
            port_num: IB_PORT,
            qp_access_flags: self.access.code(),
            ..Default::default()
        };
        let mask = ibv_qp_attr_mask::IBV_QP_STATE
            | ibv_qp_attr_mask::IBV_QP_PKEY_INDEX
            | ibv_qp_attr_mask::IBV_QP_PORT
            | ibv_qp_attr_mask::IBV_QP_ACCESS_FLAGS;
        let errno = unsafe { ibv_modify_qp(self.qp.qp, &mut attr as *mut _, mask.0 as i32) };
        if errno != 0 {
            return Err(io::Error::from_raw_os_error(errno));
        }

        // Transition to ready to receive
        let mut attr = ibv_qp_attr {
            qp_state: ibv_qp_state::IBV_QPS_RTR,
            path_mtu: self.mtu.code() as ibv_mtu,
            dest_qp_num: remote.num,
            rq_psn: self.recv_psn.code(),
            max_dest_rd_atomic: 1,
            min_rnr_timer: self.min_rnr_timer.code(), // todo: rnr timer is advertised?
            ah_attr: ibv_ah_attr {
                dlid: remote.lid,
                is_global: 0,
                sl: 0,
                src_path_bits: 0,
                port_num: IB_PORT,
                ..Default::default()
            },
            ..Default::default()
        };
        let mask = ibv_qp_attr_mask::IBV_QP_STATE
            | ibv_qp_attr_mask::IBV_QP_AV
            | ibv_qp_attr_mask::IBV_QP_PATH_MTU
            | ibv_qp_attr_mask::IBV_QP_DEST_QPN
            | ibv_qp_attr_mask::IBV_QP_RQ_PSN
            | ibv_qp_attr_mask::IBV_QP_MAX_DEST_RD_ATOMIC
            | ibv_qp_attr_mask::IBV_QP_MIN_RNR_TIMER;

        let errno = unsafe { ibv_modify_qp(self.qp.qp, &mut attr as *mut _, mask.0 as i32) };
        if errno != 0 {
            return Err(io::Error::from_raw_os_error(errno));
        }

        // Transition to ready to send
        let mut attr = ibv_qp_attr {
            qp_state: ibv_qp_state::IBV_QPS_RTS,
            timeout: self.ack_timeout.code(),
            retry_cnt: self.max_ack_retries.retries(),
            rnr_retry: self.max_rnr_retries.code(),
            max_rd_atomic: 1,
            sq_psn: self.send_psn.code(),
            ..Default::default()
        };
        let mask = ibv_qp_attr_mask::IBV_QP_STATE
            | ibv_qp_attr_mask::IBV_QP_TIMEOUT
            | ibv_qp_attr_mask::IBV_QP_RETRY_CNT
            | ibv_qp_attr_mask::IBV_QP_RNR_RETRY
            | ibv_qp_attr_mask::IBV_QP_MAX_QP_RD_ATOMIC
            | ibv_qp_attr_mask::IBV_QP_SQ_PSN;
        let errno = unsafe { ibv_modify_qp(self.qp.qp, &mut attr as *mut _, mask.0 as i32) };
        if errno != 0 {
            return Err(io::Error::from_raw_os_error(errno));
        }

        Ok(self.qp)
    }
}
