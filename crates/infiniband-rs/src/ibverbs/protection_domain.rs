use crate::ibverbs::completion_queue::CompletionQueue;
use crate::ibverbs::context::Context;
use crate::ibverbs::memory_region::MemoryRegion;
use crate::ibverbs::queue_pair_builder::{AccessFlags, QueuePairBuilder};
use ibverbs_sys::*;
use std::io;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ProtectionDomain {
    pub(super) inner: Arc<ProtectionDomainInner>,
}

impl ProtectionDomain {
    pub(super) fn allocate(context: Context) -> io::Result<Self> {
        let pd = unsafe { ibv_alloc_pd(context.inner.ctx) };
        if pd.is_null() {
            Err(io::Error::other(io::Error::last_os_error()))
        } else {
            log::debug!("IbvProtectionDomain allocated");
            Ok(ProtectionDomain {
                inner: Arc::new(ProtectionDomainInner { context, pd }),
            })
        }
    }

    pub fn context(&self) -> &Context {
        &self.inner.context
    }

    /// Registers memory with the given access flags.
    ///
    /// # Safety
    /// If the memory region registered has remote write access the memory can be DMA aliased mutably
    /// by remote peers. It can change at any point so Rust aliasing rules on the memory must be enforced
    /// manually by the user.
    pub unsafe fn register_mr_with_permissions(
        &self,
        address: *mut u8,
        length: usize,
        access_flags: ibv_access_flags,
    ) -> io::Result<MemoryRegion> {
        unsafe {
            MemoryRegion::register_with_permissions(
                self.inner.clone(),
                address,
                length,
                access_flags,
            )
        }
    }

    /// # Safety
    /// The user is responsible for ensuring the memory registered remains allocated
    /// as long as it is used in rdma operations.
    pub fn register_local_mr(&self, address: *mut u8, length: usize) -> io::Result<MemoryRegion> {
        let mr = unsafe {
            self.register_mr_with_permissions(
                address,
                length,
                AccessFlags::new().with_local_write().into(),
            )?
        };

        Ok(mr)
    }

    /// # Safety
    /// If the memory region registered has remote write access the memory can be DMA aliased mutably
    /// by remote peers. It can change at any point so Rust aliasing rules on the memory must be enforced
    /// manually by the user.
    pub unsafe fn register_shared_mr(
        &self,
        address: *mut u8,
        length: usize,
    ) -> io::Result<MemoryRegion> {
        let mr = unsafe {
            self.register_mr_with_permissions(
                address,
                length,
                AccessFlags::new()
                    .with_local_write()
                    .with_remote_read()
                    .with_remote_write()
                    .into(),
            )?
        };

        Ok(mr)
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
    /// If the memory region registered has remote write access the memory can be DMA aliased mutably
    /// by remote peers. It can change at any point so Rust aliasing rules on the memory must be enforced
    /// manually by the user.
    pub unsafe fn register_dmabuf(
        &self,
        fd: i32,
        offset: u64,
        length: usize,
        iova: u64,
        access_flags: ibv_access_flags,
    ) -> io::Result<MemoryRegion> {
        MemoryRegion::register_dmabuf(self.inner.clone(), fd, offset, length, iova, access_flags)
    }

    pub fn register_local_dmabuf(
        &self,
        fd: i32,
        offset: u64,
        length: usize,
        iova: u64,
    ) -> io::Result<MemoryRegion> {
        unsafe {
            self.register_dmabuf(
                fd,
                offset,
                length,
                iova,
                AccessFlags::new().with_local_write().into(),
            )
        }
    }

    /// # Safety
    /// If the memory region registered has remote write access the memory can be DMA aliased mutably
    /// by remote peers. It can change at any point so Rust aliasing rules on the memory must be enforced
    /// manually by the user.
    pub unsafe fn register_shared_dmabuf(
        &self,
        fd: i32,
        offset: u64,
        length: usize,
        iova: u64,
    ) -> io::Result<MemoryRegion> {
        unsafe {
            self.register_dmabuf(
                fd,
                offset,
                length,
                iova,
                AccessFlags::new()
                    .with_local_write()
                    .with_remote_read()
                    .with_remote_write()
                    .into(),
            )
        }
    }

    pub fn create_qp(
        &self,
        send_cq: &CompletionQueue,
        receive_cq: &CompletionQueue,
    ) -> QueuePairBuilder {
        QueuePairBuilder::new(
            self.inner.clone(),
            send_cq.inner.clone(),
            receive_cq.inner.clone(),
        )
    }
}

pub(super) struct ProtectionDomainInner {
    pub(super) context: Context,
    pub(super) pd: *mut ibv_pd,
}

unsafe impl Sync for ProtectionDomainInner {}
unsafe impl Send for ProtectionDomainInner {}

impl Drop for ProtectionDomainInner {
    fn drop(&mut self) {
        log::debug!("IbvProtectionDomain deallocated");
        let pd = self.pd;
        let errno = unsafe { ibv_dealloc_pd(self.pd) };
        if errno != 0 {
            let debug_text = format!("{:?}", self);
            let e = io::Error::from_raw_os_error(errno);
            log::error!(
                "({debug_text}) -> Failed to deallocate protection domain with `ibv_dealloc_pd({pd:p})`: {e}"
            );
        }
    }
}

impl std::fmt::Debug for ProtectionDomainInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IbvProtectionDomainInner")
            .field("handle", &(unsafe { *self.pd }).handle)
            .field("context", &self.context)
            .finish()
    }
}
