//! Scatter and Gather Elements (SGE).
//!
//! # The Dual Role of SGEs
//!
//! Scatter and Gather Elements serve two critical purposes in this library:
//! 1.  **Data Layout**: They define how the hardware should treat non-contiguous memory as a continuous network stream.
//! 2.  **Safety Enforcement**: They act as the bridge between Rust's borrow checker and the hardware's asynchronous operations.
//!
//! ## 1. Data Layout
//!
//! To understand the naming convention ("Scatter" vs "Gather"), it is helpful to view the network transmission
//! as a continuous **stream of bytes**, where buffer boundaries are not preserved on the wire.
//!
//! ### Outgoing: "Gather" (Serialization)
//! When performing outgoing operations (Send, RDMA Write), you provide a list of **Gather Elements**.
//! The NIC reads the memory specified by these elements sequentially and "gathers" them into a single,
//! continuous stream of bytes. The boundary between the first and second element is lost during serialization;
//! the receiver sees only the payload.
//!
//! ### Incoming: "Scatter" (Deserialization)
//! When setting up incoming operations (Receive, RDMA Read), you provide a list of **Scatter Elements**.
//! As the stream arrives from the network, the NIC cuts it into pieces to fit your buffers. It fills the
//! first buffer, then "scatters" the remaining data into the next, and so on.
//!
//! Because the network sees only a stream, the number and size of SGEs on the sender side do *not* need
//! to match the receiver side. Only the total byte length matters.
//!
//! ## 2. Safety Enforcement: Lifetimes and Aliasing
//!
//! A major challenge in RDMA is ensuring that memory buffers remain valid and uncorrupted while the
//! hardware accesses them asynchronously. This library solves this by enforcing Rust's borrowing rules
//! at the SGE creation level.
//!
//! ### Enforcing Liveness
//! To create an SGE, you must provide a valid Rust reference (`&[u8]` or `&mut [u8]`). The SGE struct
//! captures the lifetime (`'a`) of this reference. When you post a Work Request using these SGEs, the
//! Work Request inherits this lifetime dependency. Consequently, the Rust compiler guarantees that the
//! underlying buffer **cannot be deallocated** until the Work Request is complete.
//!
//! ### Enforcing Aliasing Rules
//! The distinction between [`GatherElement`] and [`ScatterElement`] strictly enforces memory access rules:
//!
//! *   **Immutable Access (Gather)**: Since the NIC only *reads* outgoing data, `GatherElement` takes a
//!     shared reference (`&[u8]`). This allows multiple concurrent operations to read the same buffer,
//!     but prevents the CPU from mutating it while the NIC is reading.
//! *   **Exclusive Access (Scatter)**: Since the NIC *writes* incoming data, `ScatterElement` takes a
//!     mutable reference (`&mut [u8]`). This guarantees **exclusive access**: no other part of the program
//!     (CPU or other NIC operations) can read or write to this buffer while the hardware is scheduled to fill it.

use crate::ibverbs::memory_region::MemoryRegion;
use ibverbs_sys::ibv_sge;
use std::marker::PhantomData;
use thiserror::Error;

/// A **Gather Element** for outgoing RDMA operations.
///
/// # Usage
///
/// A `GatherElement` represents a slice of registered memory that the NIC will **read from**
/// (gather data from) to send over the network. It is used in:
/// *   **Send Requests**: The data payload to be sent.
/// *   **RDMA Write Requests**: The local source data to write to a remote peer.
///
/// # Lifetime & Safety
///
/// The element holds a phantom reference to both the [`MemoryRegion`] and the data slice.
/// This ensures that:
/// 1.  The `MemoryRegion` cannot be dropped while this element exists.
/// 2.  The data buffer cannot be dropped or mutated while this element exists.
#[derive(Copy, Clone, Debug)]
#[repr(transparent)]
pub struct GatherElement<'a> {
    sge: ibv_sge,
    // SAFETY INVARIANT: SGE cannot outlive the referenced data or the memory region
    _mr_lifetime: PhantomData<&'a MemoryRegion>,
    _data_lifetime: PhantomData<&'a [u8]>,
}

/// A **Scatter Element** for incoming RDMA operations.
///
/// # Usage
///
/// A `ScatterElement` represents a slice of registered memory that the NIC will **write into**
/// (scatter data into) when receiving from the network. It is used in:
/// *   **Receive Requests**: The data payload to be received.
/// *   **RDMA Read Requests**: The local destination buffer for data read from a remote peer.
///
/// # Lifetime & Safety
///
/// The element holds a phantom reference to both the [`MemoryRegion`] and the data slice.
/// This ensures that:
/// 1.  The `MemoryRegion` cannot be dropped while this element exists.
/// 2.  The data buffer cannot be dropped or mutated while this element exists.
#[derive(Debug)]
#[repr(transparent)]
pub struct ScatterElement<'a> {
    sge: ibv_sge,
    // SAFETY INVARIANT: SGE cannot outlive the referenced data or the memory region
    _mr_lifetime: PhantomData<&'a MemoryRegion>,
    _data_lifetime: PhantomData<&'a mut [u8]>,
}

/// Errors that can occur when creating Scatter/Gather Elements.
#[derive(Debug, Error)]
pub enum ScatterGatherElementError {
    #[error("maximum length of mr slice exceeded")]
    SliceTooBig,
    #[error("slice is not within the bounds of the mr")]
    SliceNotWithinBounds,
}

impl<'a> GatherElement<'a> {
    /// Creates a new Gather Element with bounds checking.
    ///
    /// # Checks
    ///
    /// This method validates that:
    /// 1.  The `data` slice is fully contained within the `mr`'s address range.
    /// 2.  The `data` length fits in a `u32` (hardware limit).
    pub fn new(mr: &'a MemoryRegion, data: &'a [u8]) -> Result<Self, ScatterGatherElementError> {
        if data.len() > u32::MAX as usize {
            return Err(ScatterGatherElementError::SliceTooBig);
        }
        if !mr.encloses_slice(data) {
            return Err(ScatterGatherElementError::SliceNotWithinBounds);
        }

        Ok(Self::new_unchecked(mr, data))
    }

    /// Creates a new Gather Element **without** bounds checking.
    ///
    /// # Safety
    ///
    /// This method is safe to call from a Rust memory-safety perspective.
    /// If the slice is outside the MR, the hardware will detect this
    /// during RDMA operations and fail with a **Local Protection Error**.
    pub fn new_unchecked(mr: &'a MemoryRegion, data: &'a [u8]) -> Self {
        Self {
            sge: ibv_sge {
                addr: data.as_ptr() as u64,
                length: data.len() as u32,
                lkey: mr.lkey(),
            },
            _mr_lifetime: PhantomData::<&'a MemoryRegion>,
            _data_lifetime: PhantomData::<&'a [u8]>,
        }
    }
}

impl<'a> ScatterElement<'a> {
    /// Creates a new Scatter Element with bounds checking.
    ///
    /// # Checks
    ///
    /// This method validates that:
    /// 1.  The `data` slice is fully contained within the `mr`'s address range.
    /// 2.  The `data` length fits in a `u32`.
    pub fn new(
        mr: &'a MemoryRegion,
        data: &'a mut [u8],
    ) -> Result<Self, ScatterGatherElementError> {
        if data.len() > u32::MAX as usize {
            return Err(ScatterGatherElementError::SliceTooBig);
        }
        if !mr.encloses_slice(data) {
            return Err(ScatterGatherElementError::SliceNotWithinBounds);
        }

        Ok(Self::new_unchecked(mr, data))
    }

    /// Creates a new Scatter Element **without** bounds checking.
    ///
    /// # Safety
    ///
    /// This method is safe to call from a Rust memory-safety perspective.
    /// If the slice is outside the MR, the hardware will detect this
    /// during RDMA operations and fail with a **Local Protection Error**.
    pub fn new_unchecked(mr: &'a MemoryRegion, data: &'a mut [u8]) -> Self {
        Self {
            sge: ibv_sge {
                addr: data.as_ptr() as u64,
                length: data.len() as u32,
                lkey: mr.lkey(),
            },
            _mr_lifetime: PhantomData::<&'a MemoryRegion>,
            _data_lifetime: PhantomData::<&'a mut [u8]>,
        }
    }
}
