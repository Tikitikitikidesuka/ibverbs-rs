use crate::connection::connection::RemoteMr;
use crate::ibverbs::memory_region::IbvMemoryRegion;

use crate::ibverbs::scatter_gather_element::IbvScatterElement;
use bytemuck::bytes_of;

pub enum IbvConnectionMetaMessage {

}

#[derive(Debug)]
pub struct IbvConnectionMetaMemoryRegion {
    meta_mem: Box<RemoteMr>,
    meta_mr: IbvMemoryRegion,
}

impl IbvConnectionMetaMemoryRegion {
    pub fn share_remote_mr(&self) -> IbvScatterElement {
        self.meta_mr
            .prepare_scatter_element(bytes_of(self.meta_mem.as_ref()))
            .expect(
                "IbvConnection meta-memory content should always fit in scatter-gather elements",
            )
    }
}
