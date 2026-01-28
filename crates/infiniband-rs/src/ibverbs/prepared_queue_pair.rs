use crate::ibverbs::context::IB_PORT;
use crate::ibverbs::queue_pair::QueuePair;
use crate::ibverbs::queue_pair_endpoint::QueuePairEndpoint;
use ibverbs_sys::*;
use std::io;

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
    pub(super) qp: QueuePair,
    pub(super) lid: u16,

    /// Maximum simultaneous issued send work requests.
    pub(super) max_send_wr: u32,
    /// Maximum number of scatter-gather elements per send work request.
    pub(super) max_send_sge: u32,
    /// Maximum simultaneous issued receive work requests.
    pub(super) max_recv_wr: u32,
    /// Maximum number of scatter-gather elements per receive work request.
    pub(super) max_recv_sge: u32,
    /// Encoding specifying flags for the queue pair.
    pub(super) access: ibv_access_flags,
    /// Max number of retries when receiver reports it is not ready.
    pub(super) max_rnr_retries: u8,
    /// Max number of retries when receiver does not reply.
    pub(super) max_ack_retries: u8,
    /// Encoding specifying the timeout between retries when receiver reports it is not ready.
    pub(super) min_rnr_timer: u8,
    /// Encoding specifying the timeout between retries for receiver to reply.
    pub(super) ack_timeout: u8,
}

impl PreparedQueuePair {
    /// Get the network endpoint for this `QueuePair`.
    ///
    /// This endpoint will need to be communicated to the `QueuePair` on the remote end.
    pub fn endpoint(&self) -> QueuePairEndpoint {
        let num = unsafe { &*self.qp.qp }.qp_num;
        QueuePairEndpoint { num, lid: self.lid }
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
            qp_access_flags: self.access.0,
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
        let path_mtu = self.qp.pd.context.inner.query_port()?.active_mtu;
        let mut attr = ibv_qp_attr {
            qp_state: ibv_qp_state::IBV_QPS_RTR,
            path_mtu,
            dest_qp_num: remote.num,
            rq_psn: 0,
            max_dest_rd_atomic: 1,
            min_rnr_timer: self.min_rnr_timer,
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
            timeout: self.ack_timeout,
            retry_cnt: self.max_ack_retries,
            rnr_retry: self.max_rnr_retries,
            max_rd_atomic: 1,
            sq_psn: 0,
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
