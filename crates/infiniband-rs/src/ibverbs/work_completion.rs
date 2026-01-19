use crate::ibverbs::work_error::IbvWorkError;
use crate::ibverbs::work_success::IbvWorkSuccess;
use ibverbs_sys::ibv_wc;

pub type IbvWorkResult = Result<IbvWorkSuccess, IbvWorkError>;

#[derive(Copy, Clone, Debug)]
pub struct IbvWorkCompletion {
    wr_id: u64,
    result: IbvWorkResult,
}

impl IbvWorkCompletion {
    pub(super) fn new(wc: ibv_wc) -> Self {
        Self {
            wr_id: wc.wr_id(),
            result: if let Some((error_code, vendor_code)) = wc.error() {
                Err(IbvWorkError::new(error_code, vendor_code))
            } else {
                Ok(IbvWorkSuccess::new(wc.imm_data(), wc.len()))
            },
        }
    }
}

impl IbvWorkCompletion {
    pub fn wr_id(&self) -> u64 {
        self.wr_id
    }

    /*
    pub fn op_code(&self) {

        self.wc.opcode()
    }
    */

    pub fn result(&self) -> Result<IbvWorkSuccess, IbvWorkError> {
        self.result
    }
}
