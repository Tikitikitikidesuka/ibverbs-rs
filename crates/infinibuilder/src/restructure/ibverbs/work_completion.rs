use ibverbs::ibv_wc;
use crate::restructure::rdma_connection::RdmaWorkCompletion;

#[derive(Debug, Copy, Clone)]
pub struct IbvWorkCompletion {
    wc: ibv_wc
}

impl IbvWorkCompletion {
    pub(super) fn new(wc: ibv_wc) -> Self {
        Self { wc }
    }
}

impl RdmaWorkCompletion for IbvWorkCompletion {
    fn data_length(&self) -> usize {
        self.wc.len()
    }

    fn immediate_data(&self) -> Option<u32> {
        self.wc.imm_data()
    }
}