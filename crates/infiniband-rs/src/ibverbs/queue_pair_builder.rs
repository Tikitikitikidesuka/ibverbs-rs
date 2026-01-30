use crate::ibverbs::completion_queue::CompletionQueueInner;
use crate::ibverbs::prepared_queue_pair::PreparedQueuePair;
use crate::ibverbs::protection_domain::ProtectionDomainInner;
use crate::ibverbs::queue_pair::QueuePair;
use ibverbs_sys::*;
use std::ffi::c_void;
use std::sync::Arc;
use std::time::Duration;
use std::{io, ptr};

pub struct QueuePairBuilder {
    pd: Arc<ProtectionDomainInner>,
    send_cq: Arc<CompletionQueueInner>,
    recv_cq: Arc<CompletionQueueInner>,

    /// Maximum simultaneous issued send work requests.
    max_send_wr: u32,
    /// Maximum number of scatter-gather elements per send work request.
    max_send_sge: u32,
    /// Maximum simultaneous issued receive work requests.
    max_recv_wr: u32,
    /// Maximum number of scatter-gather elements per receive work request.
    max_recv_sge: u32,
    /// Encoding specifying flags for the queue pair.
    access: ibv_access_flags,
    /// Max number of retries when receiver reports it is not ready.
    max_rnr_retries: u8,
    /// Max number of retries when receiver does not reply.
    max_ack_retries: u8,
    /// Encoding specifying the timeout between retries when receiver reports it is not ready.
    min_rnr_timer: u8,
    /// Encoding specifying the timeout between retries for receiver to reply.
    ack_timeout: u8,
}

impl QueuePairBuilder {
    const DEFAULT_MAX_WR: u32 = 16;
    const DEFAULT_MAX_SGE: u32 = 16;
    const DEFAULT_MAX_RNR_RETRIES: u8 = 6;
    const DEFAULT_MAX_ACK_RETRIES: u8 = 6;
    const DEFAULT_RNR_TIMEOUT: u8 = 16;
    const DEFAULT_ACK_TIMEOUT: u8 = 4;
    const DEFAULT_ACCESS_FLAGS: ibv_access_flags = ibv_access_flags::IBV_ACCESS_LOCAL_WRITE;

    pub(super) fn new(
        pd: Arc<ProtectionDomainInner>,
        send_cq: Arc<CompletionQueueInner>,
        recv_cq: Arc<CompletionQueueInner>,
    ) -> Self {
        Self {
            pd,
            send_cq,
            recv_cq,
            max_send_wr: Self::DEFAULT_MAX_WR,
            max_send_sge: Self::DEFAULT_MAX_SGE,
            max_recv_wr: Self::DEFAULT_MAX_WR,
            max_recv_sge: Self::DEFAULT_MAX_SGE,
            access: Self::DEFAULT_ACCESS_FLAGS,
            max_rnr_retries: Self::DEFAULT_MAX_RNR_RETRIES,
            max_ack_retries: Self::DEFAULT_MAX_ACK_RETRIES,
            min_rnr_timer: Self::DEFAULT_RNR_TIMEOUT,
            ack_timeout: Self::DEFAULT_ACK_TIMEOUT,
        }
    }

    /// # Errors
    ///  - `EINVAL`: Invalid `ProtectionDomain`, sending or receiving `Context`, or invalid value
    ///    provided in `max_send_wr`, `max_recv_wr`, or in `max_inline_data`.
    ///  - `ENOMEM`: Not enough resources to complete this operation.
    ///  - `ENOSYS`: QP with this Transport Service Type isn't supported by this RDMA device.
    ///  - `EPERM`: Not enough permissions to create a QP with this Transport Service Type.
    pub fn build(&self) -> io::Result<PreparedQueuePair> {
        let mut attr = ibv_qp_init_attr {
            qp_context: ptr::null::<c_void>() as *mut _,
            send_cq: self.send_cq.cq as *const _ as *mut _,
            recv_cq: self.recv_cq.cq as *const _ as *mut _,
            srq: ptr::null::<ibv_srq>() as *mut _,
            cap: ibv_qp_cap {
                max_send_wr: self.max_send_wr,
                max_recv_wr: self.max_recv_wr,
                max_send_sge: self.max_send_sge,
                max_recv_sge: self.max_recv_sge,
                max_inline_data: 0,
            },
            qp_type: ibv_qp_type::IBV_QPT_RC,
            sq_sig_all: 0,
        };

        let qp = unsafe { ibv_create_qp(self.pd.pd, &mut attr as *mut _) };
        if qp.is_null() {
            Err(io::Error::last_os_error())
        } else {
            log::debug!("IbvQueuePair created");
            Ok(PreparedQueuePair {
                qp: QueuePair {
                    pd: self.pd.clone(),
                    send_cq: self.send_cq.clone(),
                    recv_cq: self.recv_cq.clone(),
                    qp,
                },
                lid: self.pd.context.inner.query_port()?.lid,

                max_send_wr: self.max_send_wr,
                max_send_sge: self.max_send_sge,
                max_recv_wr: self.max_recv_wr,
                max_recv_sge: self.max_recv_sge,
                access: self.access,
                max_rnr_retries: self.max_rnr_retries,
                max_ack_retries: self.max_ack_retries,
                min_rnr_timer: self.min_rnr_timer,
                ack_timeout: self.ack_timeout,
            })
        }
    }

    pub fn with_max_send_wrs(&mut self, max_send_wr: u32) -> &mut Self {
        self.max_send_wr = max_send_wr;
        self
    }

    pub fn with_max_send_sges(&mut self, max_send_sge: u32) -> &mut Self {
        self.max_send_sge = max_send_sge;
        self
    }

    pub fn with_max_recv_wrs(&mut self, max_recv_wr: u32) -> &mut Self {
        self.max_recv_wr = max_recv_wr;
        self
    }

    pub fn with_max_recv_sges(&mut self, max_recv_sge: u32) -> &mut Self {
        self.max_recv_sge = max_recv_sge;
        self
    }

    pub fn with_access_flags(&mut self, access_flags: AccessFlags) -> &mut Self {
        self.access = access_flags.inner;
        self
    }

    pub fn with_min_rnr_timer(&mut self, timer: RnrTimer) -> &mut Self {
        self.min_rnr_timer = timer.code();
        self
    }

    pub fn with_ack_timeout(&mut self, timeout: AckTimeout) -> &mut Self {
        self.ack_timeout = timeout.code();
        self
    }

    pub fn with_max_ack_retries(&mut self, retries: AckRetries) -> &mut Self {
        self.max_ack_retries = retries.code();
        self
    }

    pub fn with_max_rnr_retries(&mut self, retries: RnrRetries) -> &mut Self {
        self.max_rnr_retries = retries.code();
        self
    }
}






