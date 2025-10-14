use derivative::Derivative;
use ibverbs::{MemoryRegion, RemoteMemoryRegion};
use serde::{Deserialize, Serialize};
// TODO: AS OF NOW, THERE IS NO WAY OF DEREGISTERING IN `rust-ibverbs`...
// TODO: IT WOULD BE NICE TO ADD A DROP IMPL TO MR AND MAKE IT DEREGISTER

#[derive(Derivative)]
#[derivative(Debug)]
pub struct IbvMemoryRegion {
    pub(super) length: usize,
    #[derivative(Debug = "ignore")]
    pub(super) mr: MemoryRegion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IbvRemoteMemoryRegion {
    pub(super) length: usize,
    pub(super) rmr: RemoteMemoryRegion,
}

impl IbvMemoryRegion {
    pub fn remote(&self) -> IbvRemoteMemoryRegion {
        IbvRemoteMemoryRegion {
            length: self.length,
            rmr: self.mr.remote(),
        }
    }
}
