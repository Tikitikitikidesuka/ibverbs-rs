use crate::ibverbs::work_error::WorkError;
use crate::ibverbs::work_success::WorkSuccess;
use ibverbs_sys::ibv_wc;

pub type WorkResult = Result<WorkSuccess, WorkError>;

#[derive(Copy, Clone, Debug)]
pub struct WorkCompletion {
    wr_id: u64,
    result: WorkResult,
}

impl WorkCompletion {
    pub(super) fn new(wc: ibv_wc) -> Self {
        Self {
            wr_id: wc.wr_id(),
            result: if let Some((error_code, vendor_code)) = wc.error() {
                Err(WorkError::new(error_code, vendor_code))
            } else {
                Ok(WorkSuccess::new(wc.imm_data(), wc.len()))
            },
        }
    }
}

impl WorkCompletion {
    pub fn wr_id(&self) -> u64 {
        self.wr_id
    }

    pub fn result(&self) -> Result<WorkSuccess, WorkError> {
        self.result
    }
}
