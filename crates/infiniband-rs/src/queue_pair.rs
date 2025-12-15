use crate::protection_domain::IbvProtectionDomainInner;
use ibverbs_sys::*;
use std::fmt::Debug;
use std::io;
use std::sync::Arc;

pub struct IbvQueuePair {
    pub(super) pd: Arc<IbvProtectionDomainInner>,
    pub(super) qp: *mut ibv_qp,
}

unsafe impl Send for IbvQueuePair {}
unsafe impl Sync for IbvQueuePair {}

impl Drop for IbvQueuePair {
    fn drop(&mut self) {
        let qp = self.qp;
        let debug_text = format!("{:?}", self);
        let errno = unsafe { ibv_destroy_qp(self.qp) };
        if errno != 0 {
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
}
