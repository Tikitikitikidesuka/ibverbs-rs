use crate::ibverbs::scatter_gather_element::{
    GatherElement, ScatterElement, ScatterGatherElementError,
};
use crate::network::memory_region::NodeMemoryRegion;
use crate::network::node::Rank;

/// A node scatter element is not tied to a connection's memory region yet
/// since the rdma operation methods choose which connection to send
/// this through.
/// This allows reusability of a single NodeScatterElement over different connections
/// of the node by not tying it to a specific one.
#[derive(Debug, Copy, Clone)]
pub struct NodeScatterElement<'a> {
    mr: &'a NodeMemoryRegion,
    data: &'a [u8],
}

impl<'a> NodeScatterElement<'a> {
    pub(super) fn new(mr: &'a NodeMemoryRegion, data: &'a [u8]) -> Self {
        Self { mr, data }
    }

    pub(super) fn bind(&self, rank: Rank) -> Result<ScatterElement<'a>, ScatterGatherElementError> {
        // todo: treat error rank not in range
        self.mr
            .connection_mrs
            .get(rank)
            .unwrap()
            .prepare_scatter_element(self.data)
    }
}

/// A node gather element is not tied to a connection's memory region yet
/// since the rdma operation methods choose which connection to receive
/// this from.
/// This allows reusability of a single NodeGatherElement over different connections
/// of the node by not tying it to a specific one.
#[derive(Debug)]
pub struct NodeGatherElement<'a> {
    mr: &'a NodeMemoryRegion,
    data: &'a mut [u8],
}

impl<'a> NodeGatherElement<'a> {
    pub(super) fn new(mr: &'a NodeMemoryRegion, data: &'a mut [u8]) -> Self {
        Self { mr, data }
    }

    pub(super) fn bind(
        &'_ mut self,
        rank: Rank,
    ) -> Result<GatherElement<'_>, ScatterGatherElementError> {
        self.mr
            .connection_mrs
            .get(rank)
            .unwrap()
            .prepare_gather_element(self.data)
    }
}
