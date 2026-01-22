use crate::ibverbs::memory_region::MemoryRegion;
use crate::ibverbs::scatter_gather_element::ScatterGatherElementError;
use crate::network::memory_region::NodeMemoryRegion;
use ibverbs_sys::ibv_sge;
use std::marker::PhantomData;

/// This version of the GatherElement is tied to a network memory region
/// When the rdma operation decides what connection the operation is going to be run on
/// it chooses the right memory region from the node memory region for it
#[derive(Debug)]
#[repr(transparent)]
pub struct NodeScatterElement<'a> {
    sge: ibv_sge,
    // SAFETY INVARIANT: SGE cannot outlive the referenced data or the memory region
    _mr_lifetime: PhantomData<&'a NodeMemoryRegion>,
    _data_lifetime: PhantomData<&'a mut [u8]>,
}

impl<'a> NodeScatterElement<'a> {
    pub(super) fn new(
        mr: &'a NodeMemoryRegion,
        data: &'a [u8],
    ) -> Result<Self, ScatterGatherElementError> {
        let data_length = data
            .len()
            .try_into()
            .map_err(|_| ScatterGatherElementError::SliceTooBig)?;

        Ok(Self {
            sge: ibv_sge {
                addr: data.as_ptr() as u64,
                length: data_length,
                lkey: 0, // Unset for now. Will be set when connection is decided
            },
            _mr_lifetime: PhantomData::<&'a MemoryRegion>,
            _data_lifetime: PhantomData::<&'a [u8]>,
        })
    }
}
