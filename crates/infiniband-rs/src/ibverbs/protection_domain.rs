use crate::ibverbs::completion_queue::IbvCompletionQueue;
use crate::ibverbs::context::IbvContextInner;
use crate::ibverbs::memory_region::IbvMemoryRegion;
use crate::ibverbs::queue_pair_builder::IbvRcQueuePairBuilder;
use ibverbs_sys::*;
use std::io;
use std::sync::Arc;

#[derive(Debug)]
pub struct IbvProtectionDomain {
    inner: Arc<IbvProtectionDomainInner>,
}

impl IbvProtectionDomain {
    pub(super) fn allocate(context: Arc<IbvContextInner>) -> io::Result<Self> {
        let pd = unsafe { ibv_alloc_pd(context.ctx) };
        if pd.is_null() {
            Err(io::Error::other(io::Error::last_os_error()))
        } else {
            Ok(IbvProtectionDomain {
                inner: Arc::new(IbvProtectionDomainInner { context, pd }),
            })
        }
    }

    /// Registers memory with the given access flags.
    ///
    /// # Safety
    /// The user is responsible for ensuring the memory registered
    /// is not deallocated for as long as it is registered.
    pub unsafe fn register_mr_with_permissions(
        &self,
        address: *mut u8,
        length: usize,
        access_flags: ibv_access_flags,
    ) -> io::Result<IbvMemoryRegion> {
        unsafe {
            IbvMemoryRegion::register_with_permissions(
                self.inner.clone(),
                address,
                length,
                access_flags,
            )
        }
    }

    /// Registers a DMA-BUF with the given access flags.
    ///
    /// # Arguments
    /// * `fd` - The file descriptor of the DMA-BUF to be registered.
    /// * `offset`, `len` - The MR starts at `offset` of the dma-buf and its size is `len`.
    /// * `iova` - The argument iova specifies the virtual base address of the MR when accessed through a lkey or rkey.
    ///   Note: `iova` must have the same page offset as `offset`
    ///
    /// # Safety
    /// The DMA-BUF and its mapped memory must not be deallocated while they remain registered.
    pub unsafe fn register_dmabuf(
        &self,
        fd: i32,
        offset: u64,
        len: usize,
        iova: u64,
        access_flags: ibv_access_flags,
    ) -> io::Result<IbvMemoryRegion> {
        unsafe {
            IbvMemoryRegion::register_dmabuf(
                self.inner.clone(),
                fd,
                offset,
                len,
                iova,
                access_flags,
            )
        }
    }

    pub fn create_qp(
        &self,
        send_cq: &IbvCompletionQueue,
        receive_cq: &IbvCompletionQueue,
    ) -> IbvRcQueuePairBuilder {
        IbvRcQueuePairBuilder::new(
            self.inner.clone(),
            send_cq.inner.clone(),
            receive_cq.inner.clone(),
        )
    }
}

pub(super) struct IbvProtectionDomainInner {
    pub(super) context: Arc<IbvContextInner>,
    pub(super) pd: *mut ibv_pd,
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
            .field("context", &self.context)
            .finish()
    }
}
