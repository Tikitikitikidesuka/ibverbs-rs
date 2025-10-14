use ibverbs::{MemoryRegion, RemoteMemoryRegion};

// TODO: AS OF NOW, THERE IS NO WAY OF DEREGISTERING IN `rust-ibverbs`...
// TODO: IT WOULD BE NICE TO ADD A DROP IMPL TO MR AND MAKE IT DEREGISTER

pub struct IbvMemoryRegion {
    pub(super) mr: MemoryRegion,
}

pub struct IbvRemoteMemoryRegion {
    pub(super) rmr: RemoteMemoryRegion,
}

impl IbvMemoryRegion {
    pub fn remote(&self) -> IbvRemoteMemoryRegion {
        IbvRemoteMemoryRegion {
            rmr: self.mr.remote(),
        }
    }
}