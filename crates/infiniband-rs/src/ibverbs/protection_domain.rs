use crate::ibverbs::access_config::AccessFlags;
use crate::ibverbs::context::Context;
use crate::ibverbs::error::{IbvError, IbvResult};
use crate::ibverbs::queue_pair::QueuePair;
use crate::ibverbs::queue_pair::builder::QueuePairBuilder;
use crate::ibverbs::queue_pair::builder::queue_pair_builder::SetPd;
use ibverbs_sys::*;
use std::io;
use std::sync::Arc;
use crate::ibverbs::memory::MemoryRegion;

#[derive(Debug, Clone)]
pub struct ProtectionDomain {
    pub(super) inner: Arc<ProtectionDomainInner>,
}

impl ProtectionDomain {
    pub fn allocate(context: &Context) -> IbvResult<ProtectionDomain> {
        let pd = unsafe { ibv_alloc_pd(context.inner.ctx) };
        if pd.is_null() {
            Err(IbvError::from_errno_with_msg(
                io::Error::last_os_error().raw_os_error().unwrap(),
                "Failed to allocate protection domain",
            ))
        } else {
            log::debug!("ProtectionDomain allocated");
            Ok(ProtectionDomain {
                inner: Arc::new(ProtectionDomainInner {
                    context: context.clone(),
                    pd,
                }),
            })
        }
    }

    pub fn context(&self) -> &Context {
        &self.inner.context
    }

    pub fn create_qp(&self) -> QueuePairBuilder<'_, '_, '_, SetPd> {
        QueuePair::builder().pd(self)
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
        access_flags: AccessFlags,
    ) -> IbvResult<MemoryRegion> {
        unsafe { MemoryRegion::register_mr_with_access(self, address, length, access_flags) }
    }

    /// # Safety
    /// The user is responsible for ensuring the memory registered remains allocated
    /// as long as it is used in rdma operations.
    pub fn register_local_mr(&self, address: *mut u8, length: usize) -> IbvResult<MemoryRegion> {
        MemoryRegion::register_local_mr(self, address, length)
    }

    /// # Safety
    /// The user is responsible for ensuring the memory registered remains allocated
    /// as long as it is used in rdma operations.
    pub fn register_local_mr_slice(&self, mem: &[u8]) -> IbvResult<MemoryRegion> {
        MemoryRegion::register_local_mr(self, mem.as_ptr() as *mut u8, mem.len())
    }

    /// # Safety
    /// If the memory region registered has remote write access the memory can be DMA aliased mutably
    /// by remote peers. It can change at any point so Rust aliasing rules on the memory must be enforced
    /// manually by the user.
    pub unsafe fn register_shared_mr(
        &self,
        address: *mut u8,
        length: usize,
    ) -> IbvResult<MemoryRegion> {
        unsafe { MemoryRegion::register_shared_mr(self, address, length) }
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
        access_flags: AccessFlags,
    ) -> IbvResult<MemoryRegion> {
        unsafe { MemoryRegion::register_dmabuf_mr_with_access(self, fd, offset, length, iova, access_flags) }
    }

    pub fn register_local_dmabuf(
        &self,
        fd: i32,
        offset: u64,
        length: usize,
        iova: u64,
    ) -> IbvResult<MemoryRegion> {
        MemoryRegion::register_local_dmabuf_mr(self, fd, offset, length, iova)
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
    ) -> IbvResult<MemoryRegion> {
        unsafe { MemoryRegion::register_shared_dmabuf_mr(self, fd, offset, length, iova) }
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
        log::debug!("ProtectionDomain deallocated");
        let errno = unsafe { ibv_dealloc_pd(self.pd) };
        if errno != 0 {
            let error =
                IbvError::from_errno_with_msg(errno, "Failed to deallocate protection domain");
            log::error!("{error}");
        }
    }
}

impl std::fmt::Debug for ProtectionDomainInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProtectionDomainInner")
            .field("handle", &(unsafe { *self.pd }).handle)
            .field("context", &self.context)
            .finish()
    }
}
