use crate::restructure::rdma_connection::RdmaWorkCompletion;
use ibverbs::ibv_wc;
use std::fmt::Debug;

#[derive(Debug, Copy, Clone)]
pub struct IbvWorkCompletion {
    wc: ibv_wc,
}

impl IbvWorkCompletion {
    pub(super) fn new(wc: ibv_wc) -> Self {
        Self { wc }
    }
}

impl RdmaWorkCompletion for IbvWorkCompletion {
    fn local_modified_len(&self) -> usize {
        self.wc.len()
    }

    fn immediate_data(&self) -> Option<u32> {
        self.wc.imm_data()
    }
}
