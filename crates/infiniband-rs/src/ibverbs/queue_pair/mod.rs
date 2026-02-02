pub mod builder;
pub mod config;
pub mod ops;

use crate::ibverbs::completion_queue::CompletionQueue;
use crate::ibverbs::error::IbvError;
use crate::ibverbs::protection_domain::ProtectionDomain;
use ibverbs_sys::{ibv_destroy_qp, ibv_qp};
use std::fmt::Debug;

pub struct QueuePair {
    pd: ProtectionDomain,
    _send_cq: CompletionQueue,
    _recv_cq: CompletionQueue,
    qp: *mut ibv_qp,
}

unsafe impl Send for QueuePair {}
unsafe impl Sync for QueuePair {}

impl Drop for QueuePair {
    fn drop(&mut self) {
        log::debug!("QueuePair destroyed");
        let qp = self.qp;
        let errno = unsafe { ibv_destroy_qp(self.qp) };
        if errno != 0 {
            let debug_text = format!("{:?}", self);
            let error = IbvError::from_errno_with_msg(errno, "Failed to destroy queue pair");
            log::error!("({debug_text}) -> {error}");
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
