use crate::ibverbs::access_config::AccessFlags;
use crate::ibverbs::device::Context;
use crate::ibverbs::error::{IbvError, IbvResult};
use crate::ibverbs::memory::MemoryRegion;
use crate::ibverbs::queue_pair::QueuePair;
use crate::ibverbs::queue_pair::builder::QueuePairBuilder;
use crate::ibverbs::queue_pair::builder::queue_pair_builder::SetPd;
use ibverbs_sys::*;
use std::io;
use std::sync::Arc;

/// A Protection Domain (PD) groups RDMA resources that are allowed to interact with each other.
///
/// Memory regions, queue pairs, and other RDMA objects created under the same protection domain
/// can reference one another in work requests. Objects from different protection domains cannot
/// be mixed — the hardware enforces this isolation.
///
/// A `ProtectionDomain` is internally reference-counted ([`Arc`]) and can be cloned cheaply.
/// The underlying hardware resource is deallocated when the last clone is dropped.
///
/// # Creating a Protection Domain
///
/// Allocate from an open device [`Context`]:
///
/// ```no_run
/// # use ibverbs_rs::ibverbs;
/// let devices = ibverbs::list_devices()?;
/// let context = devices[0].open()?;
/// let pd = context.allocate_pd()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// [`Arc`]: std::sync::Arc
#[derive(Debug, Clone)]
pub struct ProtectionDomain {
    pub(super) inner: Arc<ProtectionDomainInner>,
}

impl ProtectionDomain {
    /// Allocates a new protection domain on the given device context.
    ///
    /// This is the low-level allocation method. Prefer [`Context::allocate_pd`] for convenience.
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

    /// Returns a reference to the device [`Context`] this protection domain belongs to.
    pub fn context(&self) -> &Context {
        &self.inner.context
    }

    /// Returns a [`QueuePairBuilder`] with this protection domain already set.
    ///
    /// This is a convenience shorthand for `QueuePair::builder().pd(self)`.
    pub fn create_qp(&self) -> QueuePairBuilder<'_, '_, '_, SetPd> {
        QueuePair::builder().pd(self)
    }

    /// Registers a memory region with the given [`AccessFlags`].
    ///
    /// This is the most flexible registration method — prefer [`register_local_mr`](Self::register_local_mr)
    /// or [`register_shared_mr`](Self::register_shared_mr) when the access pattern is known upfront.
    ///
    /// # Safety
    /// If the access flags include remote write, remote peers can write to this memory at any time
    /// via one-sided operations, bypassing Rust's borrow checker. The caller must manually ensure
    /// that Rust's aliasing rules are upheld for the entire lifetime of the returned [`MemoryRegion`].
    pub unsafe fn register_mr_with_permissions(
        &self,
        address: *mut u8,
        length: usize,
        access_flags: AccessFlags,
    ) -> IbvResult<MemoryRegion> {
        unsafe { MemoryRegion::register_mr_with_access(self, address, length, access_flags) }
    }

    /// Registers a local memory region for RDMA operations.
    ///
    /// The registered region has local write access only — remote peers cannot read from or
    /// write to it directly. This means it can only be used in two-sided operations
    /// (send/receive).
    ///
    /// # Safety
    /// The caller is responsible for ensuring the memory at `address` remains allocated and
    /// valid for `length` bytes for the lifetime of the returned [`MemoryRegion`].
    pub fn register_local_mr(&self, address: *mut u8, length: usize) -> IbvResult<MemoryRegion> {
        MemoryRegion::register_local_mr(self, address, length)
    }

    /// Registers a slice as a local memory region for RDMA operations.
    ///
    /// This is a convenience wrapper around [`register_local_mr`](Self::register_local_mr) that
    /// takes a slice instead of a raw pointer and length.
    ///
    /// # Safety
    /// The caller is responsible for ensuring the slice remains allocated for the lifetime of
    /// the returned [`MemoryRegion`].
    pub fn register_local_mr_slice(&self, mem: &[u8]) -> IbvResult<MemoryRegion> {
        MemoryRegion::register_local_mr(self, mem.as_ptr() as *mut u8, mem.len())
    }

    /// Registers a memory region with both local and remote access (read and write).
    ///
    /// The registered region can be used in both two-sided (send/receive) and one-sided
    /// (RDMA read/write) operations.
    ///
    /// # Safety
    /// Remote peers can read from and write to this memory at any time via one-sided operations,
    /// bypassing Rust's borrow checker. The caller must manually ensure that Rust's aliasing rules
    /// are upheld for the entire lifetime of the returned [`MemoryRegion`].
    pub unsafe fn register_shared_mr(
        &self,
        address: *mut u8,
        length: usize,
    ) -> IbvResult<MemoryRegion> {
        unsafe { MemoryRegion::register_shared_mr(self, address, length) }
    }

    /// Registers a DMA-BUF with the given [`AccessFlags`].
    ///
    /// This is the most flexible DMA-BUF registration method — prefer
    /// [`register_local_dmabuf`](Self::register_local_dmabuf) or
    /// [`register_shared_dmabuf`](Self::register_shared_dmabuf) when the access pattern is known
    /// upfront.
    ///
    /// # Arguments
    /// * `fd` — File descriptor of the DMA-BUF to register.
    /// * `offset` / `length` — The region starts at `offset` within the DMA-BUF and spans `length` bytes.
    /// * `iova` — Virtual base address used when accessing the region through an lkey or rkey.
    ///   Must have the same page offset as `offset`.
    ///
    /// # Safety
    /// If the access flags include remote write, remote peers can write to this memory at any time
    /// via one-sided operations, bypassing Rust's borrow checker. The caller must manually ensure
    /// that Rust's aliasing rules are upheld for the entire lifetime of the returned [`MemoryRegion`].
    pub unsafe fn register_dmabuf(
        &self,
        fd: i32,
        offset: u64,
        length: usize,
        iova: u64,
        access_flags: AccessFlags,
    ) -> IbvResult<MemoryRegion> {
        unsafe {
            MemoryRegion::register_dmabuf_mr_with_access(
                self,
                fd,
                offset,
                length,
                iova,
                access_flags,
            )
        }
    }

    /// Registers a DMA-BUF as a local memory region for RDMA operations.
    ///
    /// The registered region has local write access only — it can only be used in two-sided
    /// operations (send/receive). See [`register_dmabuf`](Self::register_dmabuf) for the
    /// argument descriptions.
    pub fn register_local_dmabuf(
        &self,
        fd: i32,
        offset: u64,
        length: usize,
        iova: u64,
    ) -> IbvResult<MemoryRegion> {
        MemoryRegion::register_local_dmabuf_mr(self, fd, offset, length, iova)
    }

    /// Registers a DMA-BUF with both local and remote access (read and write).
    ///
    /// See [`register_dmabuf`](Self::register_dmabuf) for the argument descriptions.
    ///
    /// # Safety
    /// Remote peers can read from and write to this memory at any time via one-sided operations,
    /// bypassing Rust's borrow checker. The caller must manually ensure that Rust's aliasing rules
    /// are upheld for the entire lifetime of the returned [`MemoryRegion`].
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
