use crate::ibverbs::memory_region::MemoryRegion;

// Each IbvNetworkMemoryRegion represents a slice of memory
// registered to each of the connections of the host
pub struct NodeMemoryRegion {
    connection_mrs: Vec<MemoryRegion>,
}
