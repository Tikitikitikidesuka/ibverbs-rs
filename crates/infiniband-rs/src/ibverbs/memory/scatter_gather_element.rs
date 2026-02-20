use crate::ibverbs::memory::MemoryRegion;
use ibverbs_sys::ibv_sge;
use std::marker::PhantomData;
use thiserror::Error;

/// A **Gather Element** for outgoing RDMA operations.
///
/// # Purpose
///
/// A `GatherElement` represents a slice of registered memory that the NIC will **read from**
/// to send over the network. The NIC "gathers" data from a list of these elements into a single
/// continuous stream.
///
/// # Usage
///
/// Use this for operations where the local node sends data:
/// *   **Send Requests**: The payload to send.
/// *   **RDMA Write**: The local source buffer.
///
/// # Safety and Lifetimes
///
/// This struct enforces Rust's borrowing rules at the hardware level:
///
/// *   **Immutable Access**: It holds a `&'a [u8]` to the data, allowing shared access but preventing mutation
///     while the operation is pending.
/// *   **Liveness**: It holds a reference to the [`MemoryRegion`], ensuring the registration
///     remains valid.
///
/// See the [memory module](crate::ibverbs::memory) for a detailed explanation of the safety architecture.
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
/// # Purpose
///
/// A `ScatterElement` represents a slice of registered memory that the NIC will **write into**
/// when receiving from the network. The NIC "scatters" the incoming stream across a list of
/// these elements.
///
/// # Usage
///
/// Use this for operations where the local node receives data:
/// *   **Receive Requests**: The buffer to fill with incoming data.
/// *   **RDMA Read**: The local destination buffer.
///
/// # Safety and Lifetimes
///
/// This struct enforces Rust's borrowing rules at the hardware level:
///
/// *   **Exclusive Access**: It holds a `&'a mut [u8]` to the data, ensuring no other part of the program
///     can read or write this memory while the NIC is writing to it.
/// *   **Liveness**: It holds a reference to the [`MemoryRegion`], ensuring the registration
///     remains valid.
///
/// See the [memory module](crate::ibverbs::memory) for a detailed explanation of the safety architecture.
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
    /// Creates a new gather element.
    ///
    /// In debug builds, this performs additional validation before constructing the element. In
    /// optimized/release builds, these validations are not executed by default because they are
    /// implemented using `debug_assert!`.
    ///
    /// # Panics
    ///
    /// Panics in debug builds if any of the following conditions are violated:
    /// 1. The `data` slice is fully contained within the `mr`'s address range.
    /// 2. The `data` length fits in a `u32` (hardware limit).
    ///
    /// For a version that always validates and returns an error instead of panicking, use
    /// [`Self::new_checked`].
    pub fn new(mr: &'a MemoryRegion, data: &'a [u8]) -> Self {
        debug_assert!(data.len() <= u32::MAX as usize);
        debug_assert!(mr.encloses_slice(data));
        Self::new_unchecked(mr, data)
    }

    /// Creates a new Gather Element with bounds checking.
    ///
    /// # Checks
    ///
    /// This method validates that:
    /// 1.  The `data` slice is fully contained within the `mr`'s address range.
    /// 2.  The `data` length fits in a `u32` (hardware limit).
    pub fn new_checked(
        mr: &'a MemoryRegion,
        data: &'a [u8],
    ) -> Result<Self, ScatterGatherElementError> {
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
    /// Creates a new gather element.
    ///
    /// In debug builds, this performs additional validation before constructing the element. In
    /// optimized/release builds, these validations are not executed by default because they are
    /// implemented using `debug_assert!`.
    ///
    /// # Panics
    ///
    /// Panics in debug builds if any of the following conditions are violated:
    /// 1. The `data` slice is fully contained within the `mr`'s address range.
    /// 2. The `data` length fits in a `u32` (hardware limit).
    ///
    /// For a version that always validates and returns an error instead of panicking, use
    /// [`Self::new_checked`].
    pub fn new(mr: &'a MemoryRegion, data: &'a mut [u8]) -> Self {
        debug_assert!(data.len() <= u32::MAX as usize);
        debug_assert!(mr.encloses_slice(data));
        Self::new_unchecked(mr, data)
    }

    /// Creates a new Scatter Element with bounds checking.
    ///
    /// # Checks
    ///
    /// This method validates that:
    /// 1.  The `data` slice is fully contained within the `mr`'s address range.
    /// 2.  The `data` length fits in a `u32`.
    pub fn new_checked(
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
