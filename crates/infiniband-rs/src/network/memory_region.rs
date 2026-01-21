use crate::ibverbs::memory_region::MemoryRegion;
use crate::network::scatter_gather_element::NodeScatterElement;

// Each IbvNetworkMemoryRegion represents a slice of memory
// registered to each of the connections of the host
pub struct NodeMemoryRegion {
    pub(super) connection_mrs: Vec<MemoryRegion>,
}

impl NodeMemoryRegion {
    pub(super) fn new(connection_mrs: Vec<MemoryRegion>) -> Self {
        Self { connection_mrs }
    }
}

impl NodeMemoryRegion {
    // todo: Why is connection scatter gather element not tied to mr lifetime?
    pub fn prepare_scatter_element<'a>(&'a self, data: &'a [u8]) -> NodeScatterElement<'a> {
        NodeScatterElement::new(self, data)
    }

    pub fn prepare_gather_element<'a>(&self, data: &'a [u8]) {
        todo!()
    }
}
