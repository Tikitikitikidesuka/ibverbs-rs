//! Queue pair construction and connection handshake.
//!
//! This module provides [`PreparedQueuePair`] and [`QueuePairEndpoint`], which together
//! handle the two-phase setup required to bring a queue pair to the Ready-to-Send state:
//!
//! 1. **Allocate** — call [`QueuePair::builder`](crate::ibverbs::queue_pair::QueuePair::builder)
//!    to create a [`PreparedQueuePair`] and obtain its [`QueuePairEndpoint`].
//! 2. **Exchange** — send your [`QueuePairEndpoint`] to the remote peer out-of-band (e.g., over
//!    TCP) and receive theirs.
//! 3. **Connect** — call [`PreparedQueuePair::handshake`] with the remote endpoint to drive the
//!    QP through the INIT → RTR → RTS state transitions and get back a usable [`QueuePair`].

use crate::ibverbs::access_config::AccessFlags;
use crate::ibverbs::completion_queue::CompletionQueue;
use crate::ibverbs::device::IB_PORT;
use crate::ibverbs::error::{IbvError, IbvResult};
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
    /// Configures and allocates a new Queue Pair.
    ///
    /// This builder creates the QP hardware resources.
    /// It returns a [`PreparedQueuePair`] which must be connected via a handshake before it can be used.
    ///
    /// # Arguments
    ///
    /// * `pd` — The Protection Domain this QP belongs to.
    /// * `send_cq` / `recv_cq` — The completion queues for operation results.
    /// * `access` — The operations allowed on this QP (e.g., Remote Write).
    /// * `max_*` — Hardware limits for the Work Queues.
    ///
    /// # Errors
    ///
    /// * [`IbvError::InvalidInput`] — Invalid `ProtectionDomain`, invalid Context, or invalid
    ///   configuration values (e.g., `max_send_wr` exceeds hardware limits).
    /// * [`IbvError::Resource`] — Insufficient memory or hardware resources to create the QP.
    /// * [`IbvError::Permission`] — Not enough permissions to create this type of QP.
    /// * [`IbvError::Driver`] — Underlying driver failure (e.g., `ENOSYS` if the transport type isn't supported).
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
    ) -> IbvResult<PreparedQueuePair> {
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
            Err(IbvError::from_errno_with_msg(
                io::Error::last_os_error()
                    .raw_os_error()
                    .expect("ibv_create_qp should set errno on error"),
                "Failed to create queue pair",
            ))
        } else {
            log::debug!("QueuePair created");
            Ok(PreparedQueuePair {
                qp: QueuePair {
                    pd: pd.clone(),
                    _send_cq: send_cq.clone(),
                    _recv_cq: recv_cq.clone(),
                    qp,
                },
                endpoint,

                access,
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

/// An allocated but unconnected `QueuePair`.
///
/// This struct represents a QP that has been created on the device
/// but is not yet connected to a remote peer.
///
/// # The Connection Process
///
/// 1.  **Exchange Endpoints**: Use [`endpoint()`](Self::endpoint) to get your local connection info.
///     Send this to the remote peer via an out-of-band channel (e.g., TCP). Receive the
///     peer's `QueuePairEndpoint` in return.
/// 2.  **Handshake**: Call [`handshake`](Self::handshake) with the remote peer's endpoint.
#[derive(Debug)]
pub struct PreparedQueuePair {
    qp: QueuePair,
    endpoint: QueuePairEndpoint,

    access: AccessFlags,
    max_rnr_retries: MaxRnrRetries,
    max_ack_retries: MaxAckRetries,
    min_rnr_timer: MinRnrTimer,
    ack_timeout: AckTimeout,
    mtu: MaximumTransferUnit,
    send_psn: PacketSequenceNumber,
    recv_psn: PacketSequenceNumber,
}

/// The addressing information required to connect to a specific Queue Pair.
///
/// This struct contains the Local Identifier (LID) and Queue Pair Number (QPN).
/// You must send this information to the remote peer so they can connect their QP to this one.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct QueuePairEndpoint {
    /// Queue Pair Number (QPN) — the 24-bit hardware identifier for this queue pair.
    pub num: u32,
    /// Local Identifier (LID) — the 16-bit subnet address of the port this QP is on.
    pub lid: u16,
}

impl PreparedQueuePair {
    /// Returns the network endpoint information for this `QueuePair`.
    ///
    /// Share this with the remote peer so it can connect its remote QP to this one.
    pub fn endpoint(&self) -> QueuePairEndpoint {
        self.endpoint
    }

    /// Connects this Queue Pair to a remote peer.
    ///
    /// This consumes the `PreparedQueuePair` and returns a connected `QueuePair`.
    ///
    /// # Arguments
    ///
    /// * `remote` — The endpoint information received from the remote peer.
    ///
    /// # Errors
    ///
    /// * [`IbvError::InvalidInput`] — Invalid state transition parameters (e.g., invalid port or access flags).
    /// * [`IbvError::Resource`] — Hardware resource exhaustion during state transition.
    // ibv_qp_attr_mask flag ORs are small bitmasks, well within i32 range
    #[allow(clippy::cast_possible_wrap)]
    pub fn handshake(self, remote: QueuePairEndpoint) -> IbvResult<QueuePair> {
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
            return Err(IbvError::from_errno_with_msg(
                errno,
                "Failed to set queue pair to Init state",
            ));
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
            return Err(IbvError::from_errno_with_msg(
                errno,
                "Failed to set queue pair to Ready to Receive state",
            ));
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
            return Err(IbvError::from_errno_with_msg(
                errno,
                "Failed to set queue pair to Ready to Send state",
            ));
        }

        Ok(self.qp)
    }
}
