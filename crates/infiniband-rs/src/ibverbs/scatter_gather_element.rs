use crate::connection::work_request::WorkRequestStatus;
use crate::ibverbs::memory_region::MemoryRegion;
use crate::ibverbs::work_completion::WorkResult;
use crate::ibverbs::work_error::WorkErrorCode;
use ibverbs_sys::ibv_sge;
use std::marker::PhantomData;
use thiserror::Error;

/// A **scatter element** for outgoing RDMA operations.
///
/// In raw ibverbs, scatter and gather elements are represented by the same
/// `ibv_sge` struct. Here, they are separated into `IbvScatterElement` and
/// `IbvGatherElement` based on the mutability of the data they reference and the
/// operation they represent.
///
/// An `IbvScatterElement` references a slice of a registered memory region
/// that the InfiniBand device **reads from** as part of an RDMA send or write
/// operation. It is used in RDMA send and write work requests, which contain
/// a list of scatter elements describing the slices of memory involved in
/// the operation.
///
/// The memory buffer that this entry describes must be registered until any
/// posted Work Request that uses it isn't considered outstanding anymore.
/// The order in which the RDMA device access the memory in a scatter/gather
/// list isn't defined. This means that if some of the entries overlap the
/// same memory address, the content of this address is undefined.
///
/// # Safety
///
/// The memory slice referenced by this structure must be registered until any
/// posted Work Request that uses it is not considered outstanding anymore.
/// This is ensured by setting the associated lifetime `'a` to that of the referenced
/// slice of memory.
#[derive(Copy, Clone, Debug)]
#[repr(transparent)]
pub struct ScatterElement<'a> {
    sge: ibv_sge,
    // SAFETY INVARIANT: SGE cannot outlive the referenced data or the memory region
    _mr_lifetime: PhantomData<&'a MemoryRegion>,
    _data_lifetime: PhantomData<&'a [u8]>,
}

#[derive(Debug)]
#[repr(transparent)]
pub struct GatherElement<'a> {
    sge: ibv_sge,
    // SAFETY INVARIANT: SGE cannot outlive the referenced data or the memory region
    _mr_lifetime: PhantomData<&'a MemoryRegion>,
    _data_lifetime: PhantomData<&'a mut [u8]>,
}

#[derive(Debug, Error)]
pub enum ScatterGatherElementError {
    #[error("maximum length of mr slice exceeded")]
    SliceTooBig,
    #[error("slice is not within the bounds of the mr")]
    SliceNotWithinBounds,
}

impl<'a> ScatterElement<'a> {
    pub(super) fn new(
        mr: &'a MemoryRegion,
        data: &'a [u8],
    ) -> Result<Self, ScatterGatherElementError> {
        let data_length = data
            .len()
            .try_into()
            .map_err(|_| ScatterGatherElementError::SliceTooBig)?;
        if !mr.encloses(data) {
            // todo: verify if this check is necessary
            // todo: hardware may take care of it by issuing a protection error
            // todo: if not within the registered memory boundaries
            return Err(ScatterGatherElementError::SliceNotWithinBounds);
        }

        Ok(Self {
            sge: ibv_sge {
                addr: data.as_ptr() as u64,
                length: data_length,
                lkey: mr.lkey(),
            },
            _mr_lifetime: PhantomData::<&'a MemoryRegion>,
            _data_lifetime: PhantomData::<&'a [u8]>,
        })
    }
}

impl<'a> GatherElement<'a> {
    pub(super) fn new(
        mr: &'a MemoryRegion,
        data: &'a mut [u8],
    ) -> Result<Self, ScatterGatherElementError> {
        let data_length = data
            .len()
            .try_into()
            .map_err(|_| ScatterGatherElementError::SliceTooBig)?;
        if !mr.encloses(data) {
            // todo: verify if this check is necessary
            // todo: hardware may take care of it by issuing a protection error
            // todo: if not within the registered memory boundaries
            return Err(ScatterGatherElementError::SliceNotWithinBounds);
        }

        Ok(Self {
            sge: ibv_sge {
                addr: data.as_ptr() as u64,
                length: data_length,
                lkey: mr.lkey(),
            },
            _mr_lifetime: PhantomData::<&'a MemoryRegion>,
            _data_lifetime: PhantomData::<&'a mut [u8]>,
        })
    }
}
