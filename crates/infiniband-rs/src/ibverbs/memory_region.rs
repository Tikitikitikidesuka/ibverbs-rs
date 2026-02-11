/*
//! My Rant on Memory Region safety:
//!
//! There are two types of operations in RDMA:
//! - Multiparticipant: Both nodes involved in the communication have to participate in the exchange.
//!     These are send and receive operations. The sender must send some regions of memory and the
//!     receiver must receive some regions of memory. The length of the added regions of memory and
//!     that of the received ones must match.
//! - Monoparticipant: Only one node is involved in the communication. That is write and read operations.
//!     The participant decides which local memory he is going to write into which remote region (for writes);
//!     Or which remote region he is going to read into which local memory (for reads).
//!
//! Multiparticpant operations guarantee some protocol invariants which the monoparticpant operatinos cannot
//! due to a lack of information.
//! When a send is issued, the only information required is local. That is, the node selects which local memory
//! region is going to be sent, but does not need to know anything about the remote peer's memory configuration.
//! Therefore, all the information to check whether the memory involved in the operation respects aliasing rules
//! and is alive. The same happens with a receive operation.
//!
//! How is memory registration safe in regards to multiparticipant operations?
//! The main problem with memory regions is ensuring the memory they reference is valid as long
//! as the memory region is. That is, that a memory region exists implies the buffer backing it up
//! also does.
//!
//! The simplest way of keeping this invariant is making a memory region own its memory.
//! But this is a very inflexible solution. A same buffer might be necessary to register into multiple
//! protection domains; or it might be owned by some other structure.
//!
//! Another solution, the one proposed having only multiparticipant operations in mind, is tying
//! the memory to the memory region as long as it is used. Whenever a send or receive operation is to
//! be issued, the registered memory region and its memory have to be tied in a structure that lives
//! for as long as the work request, that is, from when the operation is issued to when its polled to completion.
//! This solution ensures that when you issue a send, for example, the memory of the buffer involved in that send
//! remains allocated and valid until the send is finished.
//!
//! With this solution, registering a piece of memory into a memory region and then deallocating that memory
//! is still allowed, but what is not allowed is issuing an rdma operation that uses that memory region
//! and its memory locally (send /recv). This is safe because the NIC cannot be ordered to access
//! a memory region whose buffer has been deallocated.
//!
//! This measure however only works with multiparticipant operations. Because no information of the
//! liveliness of a remote memory region is necessary. On a write or read operation, remote memory regions
//! are involved and the lifetime of their backing buffer cannot be tied to the remote mr because that information
//! is not available locally.
//!
//! The measure can be applied partially to the local part of the operation, that is, when issuing a write operation,
//! the local memory region from which to copy the data can be tied to its local buffer, ensuring its still alive.
//! But the remote memory region could be pointing to a buffer that has been deallocated in the remote peer.
//! The same happens with read operations for the local memory region which the data will be copied into.
//!
//! So how is memory region registration safe?
//! As long as a memory region is registered with only local write permissions, it only allows multiparticipant
//! operations on it and therefore, all the safety prommised for them works while the unsafety of the monoparticpant
//! operations is dissallowed. This makes the registration of this kind of memory regino completely safe.
//!
//! For other kinds of access parameters, safety cannot be guaranteed due to the monoparticipant unsafetyness
//! being allowed.
//!
//! A secondary problem with memory regions is aliasing rules. These are already solved with the solution proposed above
//! for multiparticipant operations. Since a slice of the memory has to be tied to the memory regino before an operatino is issued
//! and is tied until complete, rust aliasing rules are guaranteed.
//! However, when a remote memory region is registered with remote write permissions, the
//! memory might change value at any point by a remote write operation. If there exists for example a reference to it
//! locally, its aliasing guarantees will be broken.
*/

//! Memory Region (MR) management.
//!
//! A [`MemoryRegion`] represents a block of memory registered with the NIC.
//!
//! # What is Registration?
//!
//! "Registration" is the process of pinning a memory buffer (preventing the OS from swapping it out)
//! and providing the NIC with a translation from virtual addresses to physical addresses.
//! This allows the NIC to access the memory directly (DMA) without CPU intervention.
//!
//! # Safety Architecture: The "Usage-Time" Guarantee
//!
//! RDMA safety is complex because the hardware accesses memory asynchronously. To ensure safety
//! without forcing the [`MemoryRegion`] to own the underlying data (which would be inflexible),
//! this library enforces safety at **usage time** via Scatter/Gather Elements (SGE).
//!
//! ## 1. Two-Sided Operations (Send/Receive)
//!
//! In Send/Receive operations, both the sender and receiver actively post Work Requests.
//!
//! *   **Sending (Gather)**: The sender *reads* from local memory to send to the network.
//!     You use [`gather_element`](MemoryRegion::gather_element) which takes a shared reference (`&[u8]`).
//! *   **Receiving (Scatter)**: The receiver *writes* incoming data into local memory.
//!     You use [`scatter_element`](MemoryRegion::scatter_element) which takes a mutable reference (`&mut [u8]`).
//!
//! **The Guarantee**: Creating an SGE requires passing a Rust slice. This binds the lifetime of the SGE
//! to the lifetime of the data. You cannot post a request if the data has been dropped, because the
//! borrow checker prevents creating the SGE.
//!
//! ## 2. One-Sided Operations (RDMA Write/Read)
//!
//! In RDMA Write/Read, one active peer accesses the passive peer's memory directly.
//!
//! *   **Active Side (Initiator)**: Safe. The initiator creates an SGE for their *local* source/dest
//!     buffer. The same lifetime guarantees apply as above.
//! *   **Passive Side (Target)**: **Unsafe**. The target does *not* post a Work Request. They
//!     simply register memory with remote access (like `remote_write`) and wait.
//!     *   There is no SGE to bind the lifetime to.
//!     *   **Aliasing**: A remote peer can write to this memory at any time. This violates Rust's
//!         memory model if safe code is simultaneously accessing it.
//!
//! # Registration & Permissions
//!
//! You can register memory with different [`AccessFlags`]
//! to control what operations are allowed:
//!
//! *   [`register_local_mr`](MemoryRegion::register_local_mr): **Safe**. Sets only local write access.
//!     Allows usage in Send, Receive, and as the *initiator* of RDMA Reads/Writes.
//! *   [`register_shared_mr`](MemoryRegion::register_shared_mr): **Unsafe**.
//!     Sets remote read and remote write access as well.
//!     Allows the memory to be the *target* of RDMA operations.
//! *   [`register_mr_with_access`](MemoryRegion::register_mr_with_access): **Unsafe**.
//!     Manual control of the memory region's access.

use crate::ibverbs::access_config::AccessFlags;
use crate::ibverbs::error::{IbvError, IbvResult};
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::ibverbs::remote_memory_region::RemoteMemoryRegion;
use crate::ibverbs::scatter_gather_element::*;
use ibverbs_sys::*;
use std::ffi::c_void;
use std::io;

/// A handle to a registered Memory Region.
///
/// This struct manages the registration lifecycle. Note that it does **not** own the underlying
/// memory buffer. Ownership and validity of the buffer are checked when creating Scatter/Gather
/// elements.
///
/// It holds a struct ([`ProtectionDomain`]) referencing the protection domain
/// under which it was registered. This guarantees that the Protection Domain remains
/// allocated as long as this Memory Region exists.
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
    /// *   `pd`: The Protection Domain to register this memory region in.
    /// *   `address`: Pointer to the start of the memory buffer to register.
    /// *   `length`: The size of the buffer in bytes.
    /// *   `access_flags`: The permissions to grant to the NIC for this memory region.
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
    /// *   `pd`: The Protection Domain.
    /// *   `address`: Pointer to the start of the memory buffer.
    /// *   `length`: The size of the buffer in bytes.
    ///
    /// # Why is this Safe?
    ///
    /// Even though this does not take ownership of the memory, it is safe because:
    /// 1.  It does not allow Remote access (no aliasing risk).
    /// 2.  To use this MR locally (Send/Recv/Write-Source), you must create an SGE.
    ///     The SGE creation requires a valid reference to the memory, proving it is still alive.
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
    /// *   `pd`: The Protection Domain.
    /// *   `address`: Pointer to the start of the memory buffer.
    /// *   `length`: The size of the buffer in bytes.
    ///
    /// # Safety
    ///
    /// This is `unsafe` because it allows remote peers to access the memory.
    /// *   **Aliasing**: The memory effectively becomes shared mutable state. It is your
    ///     responsibility to ensure aliasing rules are respected while remote peers perform
    ///     RDMA operations on it.
    /// *   **Lifetime**: You must manually ensure the memory remains allocated as long as
    ///     remote peers are actively performing RDMA operations on it.
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
    /// *   `pd`: The Protection Domain to register this memory region in.
    /// *   `fd`: The file descriptor of the DMA-BUF to be registered.
    /// *   `offset`: The start offset within the DMA-BUF file. The MR begins at this offset.
    /// *   `length`: The size of the region to register (in bytes).
    /// *   `iova`: The Input/Output Virtual Address. This is the virtual base address the NIC
    ///     will use when accessing this MR via lkey/rkey.
    ///     **Important**: `iova` must have the same page offset as `offset`.
    /// *   `access_flags`: The permissions for this memory region.
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
    /// *   `pd`: The Protection Domain.
    /// *   `fd`: The file descriptor of the DMA-BUF.
    /// *   `offset`: The start offset within the DMA-BUF file.
    /// *   `length`: The size of the region to register (in bytes).
    /// *   `iova`: The virtual base address. Must have the same page offset as `offset`.
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
    /// *   `pd`: The Protection Domain.
    /// *   `fd`: The file descriptor of the DMA-BUF.
    /// *   `offset`: The start offset within the DMA-BUF file.
    /// *   `length`: The size of the region to register (in bytes).
    /// *   `iova`: The virtual base address. Must have the same page offset as `offset`.
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
    pub fn address(&self) -> u64 {
        unsafe { (*self.mr).addr as u64 }
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
        RemoteMemoryRegion::new(self.address(), self.length(), self.rkey())
    }
}

impl MemoryRegion {
    /// Creates a **Gather Element** (for Sending/Reading) from a shared slice.
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
    pub fn gather_element<'a>(
        &'a self,
        data: &'a [u8],
    ) -> Result<GatherElement<'a>, ScatterGatherElementError> {
        GatherElement::<'a>::new(self, data)
    }

    /// Creates a **Gather Element** without immediate bounds checking.
    ///
    /// # Behavior
    ///
    /// This bypasses the software checks for:
    /// *   Memory region containment.
    /// *   Length limits (`u32`).
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
        GatherElement::<'a>::new_unchecked(self, data)
    }

    /// Creates a **Scatter Element** (for Receiving/Writing) from a mutable slice.
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
    pub fn scatter_element<'a>(
        &'a self,
        data: &'a mut [u8],
    ) -> Result<ScatterElement<'a>, ScatterGatherElementError> {
        ScatterElement::<'a>::new(self, data)
    }

    /// Creates a **Scatter Element** without immediate bounds checking.
    ///
    /// # Behavior
    ///
    /// This bypasses the software checks for:
    /// *   Memory region containment.
    /// *   Length limits (`u32`).
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
        ScatterElement::<'a>::new_unchecked(self, data)
    }

    /// Checks if the given address range is fully contained within this MR.
    pub fn encloses(&self, address: *const u8, length: usize) -> bool {
        let mr_start = self.address() as usize;
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
