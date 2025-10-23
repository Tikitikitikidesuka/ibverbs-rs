use crate::rdma_connection::RdmaWorkCompletion;
use ibverbs::ibv_wc;
use std::fmt::{Debug, Display, Formatter};

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
        self.wc.imm_data().map(|i| u32::from_be(i))
    }
}

impl Display for IbvWorkCompletion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.immediate_data() {
            None => write!(
                f,
                "IbvWorkCompletion(local_modified_len={})",
                self.local_modified_len()
            ),
            Some(d) => write!(
                f,
                "IbvWorkCompletion(immediate_data={}, local_modified_len={})",
                d,
                self.local_modified_len()
            ),
        }
    }
}
