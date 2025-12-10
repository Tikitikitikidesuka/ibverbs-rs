use crate::completion_queue::IbvCompletionQueueInner;
use crate::context::IbvContextInner;
use ibverbs_sys::*;
use std::io;
use std::sync::Arc;

#[derive(Debug)]
pub struct IbvProtectionDomain {
    inner: Arc<IbvProtectionDomainInner>,
}

impl IbvProtectionDomain {
    // TODO: VERIFY
    pub(super) fn allocate(context: Arc<IbvContextInner>) -> io::Result<Self> {
        let pd = unsafe { ibv_alloc_pd(context.ctx) };
        if pd.is_null() {
            Err(io::Error::other("obv_alloc_pd returned null"))
        } else {
            Ok(IbvProtectionDomain {
                inner: Arc::new(IbvProtectionDomainInner { _ctx: context, pd }),
            })
        }
    }
}

struct IbvProtectionDomainInner {
    _ctx: Arc<IbvContextInner>,
    pd: *mut ibv_pd,
}

unsafe impl Sync for IbvProtectionDomainInner {}
unsafe impl Send for IbvProtectionDomainInner {}

impl Drop for IbvProtectionDomainInner {
    fn drop(&mut self) {
        let pd = self.pd;
        let debug_text = format!("{:?}", self);
        let errno = unsafe { ibv_dealloc_pd(self.pd) };
        if errno != 0 {
            let e = io::Error::from_raw_os_error(errno);
            log::error!(
                "({debug_text}) -> Failed to release completion queue with `ibv_destroy_cq({pd:p})`: {e}"
            );
        }
    }
}

impl std::fmt::Debug for IbvProtectionDomainInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IbvProtectionDomainInner")
            .field("handle", &(unsafe { *self.pd }).handle)
            .field("context", &self._ctx)
            .finish()
    }
}
