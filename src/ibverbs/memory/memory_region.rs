use crate::ibverbs::access_config::AccessFlags;
use crate::ibverbs::error::{IbvError, IbvResult};
use crate::ibverbs::memory::{
    GatherElement, RemoteMemoryRegion, ScatterElement, ScatterGatherElementError,
};
use crate::ibverbs::protection_domain::ProtectionDomain;
use ibverbs_sys::*;
use std::ffi::c_void;
use std::io;

/// A handle to a registered Memory Region.
///
/// A `MemoryRegion` represents a block of memory registered with the NIC for RDMA operations.
///
/// # What is Registration?
///
/// Registration pins a memory buffer (preventing OS swapping) and provides the NIC with
/// virtual-to-physical address translation, enabling direct memory access (DMA) without CPU
/// involvement.
///
/// # Ownership Model
///
/// `MemoryRegion` does **not** own the underlying buffer. This design allows:
/// * Registering the same buffer in multiple Protection Domains.
/// * Registering memory owned by other structures.
/// * Flexible memory management strategies.
///
/// Safety is enforced at **usage time** when creating [`GatherElement`] or [`ScatterElement`]
/// instances (see the [memory module](crate::ibverbs::memory) for details).
///
/// # Registration Methods
///
/// ## Safe Registration
///
/// * [`register_local_mr`](MemoryRegion::register_local_mr) — Local write access only.
///   Safe because all operations require creating SGEs with valid Rust references.
///
/// ## Unsafe Registration
///
/// * [`register_shared_mr`](MemoryRegion::register_shared_mr) — Adds remote read/write access.
///   Unsafe because remote peers can access memory asynchronously, breaking aliasing guarantees.
/// * [`register_mr_with_access`](MemoryRegion::register_mr_with_access) — Full manual control.
///   Unsafe when remote access flags are enabled.
pub struct MemoryRegion {
    pd: ProtectionDomain,
    mr: *mut ibv_mr,
}

/// SAFETY: libibverbs components are thread safe.
unsafe impl Sync for MemoryRegion {}
/// SAFETY: libibverbs components are thread safe.
unsafe impl Send for MemoryRegion {}

impl Drop for MemoryRegion {
    fn drop(&mut self) {
        log::debug!("MemoryRegion deregistered");
        // SAFETY: self.mr is valid.
        let errno = unsafe { ibv_dereg_mr(self.mr) };
        if errno != 0 {
            let error = IbvError::from_errno_with_msg(errno, "Failed to deregister memory region");
            log::error!("{error}");
        }
    }
}

impl std::fmt::Debug for MemoryRegion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryRegion")
            .field("address", &(unsafe { (*self.mr).addr }))
            .field("length", &(unsafe { (*self.mr).length }))
            .field("handle", &(unsafe { (*self.mr).handle }))
            .field("lkey", &(unsafe { (*self.mr).lkey }))
            .field("rkey", &(unsafe { (*self.mr).rkey }))
            .field("pd", &self.pd)
            .finish()
    }
}

impl MemoryRegion {
    /// Registers a memory region with specific access permissions.
    ///
    /// # Arguments
    ///
    /// * `pd` — The Protection Domain to register this memory region in.
    /// * `address` — Pointer to the start of the memory buffer to register.
    /// * `length` — The size of the buffer in bytes.
    /// * `access_flags` — The permissions to grant to the NIC for this memory region.
    ///
    /// # Safety
    ///
    /// This function is `unsafe` because enabling remote read or write access
    /// breaks local safety guarantees. If remote access is enabled:
    /// 1.  **Aliasing**: Remote peers can modify this memory at any time.
    ///     You must manually ensure Rust's aliasing rules are respected.
    /// 2.  **Lifetime**: You must manually ensure the memory remains allocated as long as
    ///     remote peers are actively performing RDMA operations on it.
    pub unsafe fn register_mr_with_access(
        pd: &ProtectionDomain,
        address: *mut u8,
        length: usize,
        access_flags: AccessFlags,
    ) -> IbvResult<MemoryRegion> {
        // ibv_access_flags values are small bitmasks (max 31), always fit in i32
        #[allow(clippy::cast_possible_wrap)]
        let mr = unsafe {
            ibv_reg_mr(
                pd.inner.pd,
                address as *mut c_void,
                length,
                access_flags.code() as i32,
            )
        };
        if mr.is_null() {
            Err(IbvError::from_errno_with_msg(
                io::Error::last_os_error().raw_os_error().unwrap(),
                "Failed to register memory region",
            ))
        } else {
            log::debug!("MemoryRegion registered");
            Ok(MemoryRegion { pd: pd.clone(), mr })
        }
    }

    /// Registers a local memory region (Local Write access).
    ///
    /// This enables local write access only.
    ///
    /// # Arguments
    ///
    /// * `pd` — The Protection Domain.
    /// * `address` — Pointer to the start of the memory buffer.
    /// * `length` — The size of the buffer in bytes.
    ///
    /// # Why is this Safe?
    ///
    /// Even though this does not take ownership of the memory, it is safe because:
    /// 1.  It does not allow Remote access (no aliasing risk).
    /// 2.  To use this MR locally (Send/Recv/Write-Source), you must create an SGE.
    ///     The SGE creation requires a valid reference to the memory, proving it is still alive.
    // address is passed to libibverbs for registration, not locally dereferenced; hardware enforces validity
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn register_local_mr(
        pd: &ProtectionDomain,
        address: *mut u8,
        length: usize,
    ) -> IbvResult<MemoryRegion> {
        unsafe {
            Self::register_mr_with_access(
                pd,
                address,
                length,
                AccessFlags::new().with_local_write(),
            )
        }
    }

    /// Registers a shared memory region with local write and remote read and write access.
    ///
    /// # Arguments
    ///
    /// * `pd` — The Protection Domain.
    /// * `address` — Pointer to the start of the memory buffer.
    /// * `length` — The size of the buffer in bytes.
    ///
    /// # Safety
    ///
    /// This is `unsafe` because it allows remote peers to access the memory.
    /// * **Aliasing** — The memory effectively becomes shared mutable state. It is your
    ///   responsibility to ensure aliasing rules are respected while remote peers perform
    ///   RDMA operations on it.
    /// * **Lifetime** — You must manually ensure the memory remains allocated as long as
    ///   remote peers are actively performing RDMA operations on it.
    pub unsafe fn register_shared_mr(
        pd: &ProtectionDomain,
        address: *mut u8,
        length: usize,
    ) -> IbvResult<MemoryRegion> {
        unsafe {
            Self::register_mr_with_access(
                pd,
                address,
                length,
                AccessFlags::new()
                    .with_local_write()
                    .with_remote_read()
                    .with_remote_write(),
            )
        }
    }

    /// Registers a DMA-BUF with the given access flags.
    ///
    /// # Arguments
    ///
    /// * `pd` — The Protection Domain to register this memory region in.
    /// * `fd` — The file descriptor of the DMA-BUF to be registered.
    /// * `offset` — The start offset within the DMA-BUF file. The MR begins at this offset.
    /// * `length` — The size of the region to register (in bytes).
    /// * `iova` — The Input/Output Virtual Address. This is the virtual base address the NIC
    ///   will use when accessing this MR via lkey/rkey.
    ///   **Important**: `iova` must have the same page offset as `offset`.
    /// * `access_flags` — The permissions for this memory region.
    ///
    /// # Safety
    ///
    /// Same safety rules as [`register_mr_with_access`](Self::register_mr_with_access).
    /// If `access_flags` includes remote capabilities, the user must manage aliasing and lifetimes manually.
    pub unsafe fn register_dmabuf_mr_with_access(
        pd: &ProtectionDomain,
        fd: i32,
        offset: u64,
        length: usize,
        iova: u64,
        access_flags: AccessFlags,
    ) -> IbvResult<MemoryRegion> {
        // ibv_access_flags values are small bitmasks (max 31), always fit in i32
        #[allow(clippy::cast_possible_wrap)]
        let mr = unsafe {
            ibv_reg_dmabuf_mr(
                pd.inner.pd,
                offset,
                length,
                iova,
                fd,
                access_flags.code() as i32,
            )
        };

        if mr.is_null() {
            Err(IbvError::from_errno_with_msg(
                io::Error::last_os_error().raw_os_error().unwrap(),
                "Failed to register memory region",
            ))
        } else {
            log::debug!("IbvMemoryRegion registered");
            Ok(MemoryRegion { pd: pd.clone(), mr })
        }
    }

    /// Registers a DMA-BUF for local access only.
    ///
    /// # Arguments
    ///
    /// * `pd` — The Protection Domain.
    /// * `fd` — The file descriptor of the DMA-BUF.
    /// * `offset` — The start offset within the DMA-BUF file.
    /// * `length` — The size of the region to register (in bytes).
    /// * `iova` — The virtual base address. Must have the same page offset as `offset`.
    ///
    /// Safe for the same reasons as [`register_local_mr`](Self::register_local_mr): usages are gated by SGE creation.
    pub fn register_local_dmabuf_mr(
        pd: &ProtectionDomain,
        fd: i32,
        offset: u64,
        length: usize,
        iova: u64,
    ) -> IbvResult<MemoryRegion> {
        unsafe {
            Self::register_dmabuf_mr_with_access(
                pd,
                fd,
                offset,
                length,
                iova,
                AccessFlags::new().with_local_write(),
            )
        }
    }

    /// Registers a DMA-BUF for shared access.
    ///
    /// # Arguments
    ///
    /// * `pd` — The Protection Domain.
    /// * `fd` — The file descriptor of the DMA-BUF.
    /// * `offset` — The start offset within the DMA-BUF file.
    /// * `length` — The size of the region to register (in bytes).
    /// * `iova` — The virtual base address. Must have the same page offset as `offset`.
    ///
    /// # Safety
    ///
    /// Unsafe due to remote access risks. See [`register_shared_mr`](Self::register_shared_mr).
    pub unsafe fn register_shared_dmabuf_mr(
        pd: &ProtectionDomain,
        fd: i32,
        offset: u64,
        length: usize,
        iova: u64,
    ) -> IbvResult<MemoryRegion> {
        unsafe {
            Self::register_dmabuf_mr_with_access(
                pd,
                fd,
                offset,
                length,
                iova,
                AccessFlags::new()
                    .with_local_write()
                    .with_remote_read()
                    .with_remote_write(),
            )
        }
    }
}

impl MemoryRegion {
    /// Returns the Remote Key (rkey) for this MR.
    ///
    /// This key is used by remote peers to access this memory region via RDMA operations.
    pub fn rkey(&self) -> u32 {
        unsafe { *self.mr }.rkey
    }

    /// Returns the starting virtual address of the registered memory buffer.
    pub fn address(&self) -> usize {
        unsafe { (*self.mr).addr as usize }
    }

    /// Returns the length of the registered memory region in bytes.
    pub fn length(&self) -> usize {
        unsafe { (*self.mr).length }
    }

    /// Returns the Local Key (lkey) for this MR.
    ///
    /// This key is used locally in Work Requests (within Scatter/Gather Elements) to prove
    /// to the NIC that the application has the right to access this memory.
    pub fn lkey(&self) -> u32 {
        unsafe { *self.mr }.lkey
    }

    /// Returns a remote endpoint of this MR for remote peers to use in one-sided operations.
    ///
    /// This struct contains the triplet (Address, Length, RKey) needed by a remote node
    /// to perform RDMA Read or Write operations on this memory.
    ///
    /// # Warning
    ///
    /// If the peer attempts an operation (e.g., RDMA Write) that was not enabled during
    /// registration, their operation will fail with a **Remote Access Error**.
    pub fn remote(&self) -> RemoteMemoryRegion {
        RemoteMemoryRegion::new(self.address() as u64, self.length(), self.rkey())
    }
}

impl MemoryRegion {
    /// Creates a **Gather Element** (for Sending/Writing) using the "raw" constructor.
    ///
    /// # Debug checks
    ///
    /// In debug builds, this validates MR containment and the `u32` length limit and may panic if
    /// they are violated (because it uses `debug_assert!`). In release builds, these checks are
    /// not executed by default.
    pub fn gather_element<'a>(&'a self, data: &'a [u8]) -> GatherElement<'a> {
        GatherElement::new(self, data)
    }

    /// Creates a **Gather Element** (for Sending/Writing) from a shared slice.
    ///
    /// # Checks
    ///
    /// This method validates that:
    /// 1.  The slice is fully contained within this [`MemoryRegion`].
    /// 2.  The slice's length fits in a `u32` (hardware limit for a single SGE).
    ///
    /// If these checks fail, it returns an error immediately.
    ///
    /// # Safety Guarantee
    ///
    /// This takes a `&'a [u8]`, ensuring the memory is initialized and cannot be mutated
    /// while the operation is pending (Rust borrowing rules).
    pub fn gather_element_checked<'a>(
        &'a self,
        data: &'a [u8],
    ) -> Result<GatherElement<'a>, ScatterGatherElementError> {
        GatherElement::new_checked(self, data)
    }

    /// Creates a **Gather Element** without immediate bounds checking.
    ///
    /// # Behavior
    ///
    /// This bypasses the software checks for:
    /// * Memory region containment.
    /// * Length limits (`u32`).
    ///
    /// # Safety
    ///
    /// This method is safe to call. If the slice is not within the memory region, or if the
    /// length is invalid, the library will create the SGE anyway.
    ///
    /// However, the **hardware** will catch this mismatch when the Work Request is executed.
    /// The operation will fail with a **Local Protection Error**,
    /// but it will not cause Undefined Behavior.
    pub fn gather_element_unchecked<'a>(&'a self, data: &'a [u8]) -> GatherElement<'a> {
        GatherElement::new_unchecked(self, data)
    }

    /// Creates a **Scatter Element** (for Receiving/Reading) using the "raw" constructor.
    ///
    /// # Debug checks
    ///
    /// In debug builds, this validates MR containment and the `u32` length limit and may panic if
    /// they are violated (because it uses `debug_assert!`). In release builds, these checks are
    /// not executed by default.
    ///
    pub fn scatter_element<'a>(&'a self, data: &'a mut [u8]) -> ScatterElement<'a> {
        ScatterElement::new(self, data)
    }

    /// Creates a **Scatter Element** (for Receiving/Reading) from a mutable slice.
    ///
    /// # Checks
    ///
    /// This method validates that:
    /// 1.  The slice is fully contained within this [`MemoryRegion`].
    /// 2.  The slice's length fits in a `u32`.
    ///
    /// # Safety Guarantee
    ///
    /// This takes a `&'a mut [u8]`, ensuring you have exclusive access to the buffer
    /// and no other part of your program is reading it while the NIC writes to it.
    pub fn scatter_element_checked<'a>(
        &'a self,
        data: &'a mut [u8],
    ) -> Result<ScatterElement<'a>, ScatterGatherElementError> {
        ScatterElement::new_checked(self, data)
    }

    /// Creates a **Scatter Element** without immediate bounds checking.
    ///
    /// # Behavior
    ///
    /// This bypasses the software checks for:
    /// - Memory region containment.
    /// - Length limits (`u32`).
    ///
    /// # Safety
    ///
    /// This method is safe to call. If the slice is not within the memory region, or if the
    /// length is invalid, the library will create the SGE anyway.
    ///
    /// However, the **hardware** will catch this mismatch when the Work Request is executed.
    /// The operation will fail with a **Local Protection Error**,
    /// but it will not cause Undefined Behavior.
    pub fn scatter_element_unchecked<'a>(&'a self, data: &'a mut [u8]) -> ScatterElement<'a> {
        ScatterElement::new_unchecked(self, data)
    }

    /// Checks if the given address range is fully contained within this MR.
    pub fn encloses(&self, address: *const u8, length: usize) -> bool {
        let mr_start = self.address();
        let mr_end = mr_start + self.length();
        let data_start = address as usize;
        let data_end = data_start + length;
        data_start >= mr_start && data_end <= mr_end
    }

    /// Checks if the given slice is fully contained within this MR.
    pub fn encloses_slice(&self, slice: &[u8]) -> bool {
        self.encloses(slice.as_ptr(), slice.len())
    }
}
