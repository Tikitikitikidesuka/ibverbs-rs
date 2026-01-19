#[derive(Copy, Clone, Debug)]
pub struct IbvWorkSuccess {
    imm_data: Option<u32>,
    gathered_length: usize,
}

impl IbvWorkSuccess {
    pub(super) fn new(imm_data: Option<u32>, gathered_length: usize) -> Self {
        Self {
            imm_data,
            gathered_length,
        }
    }
}

impl IbvWorkSuccess {
    pub fn immediate_data(&self) -> Option<u32> {
        self.imm_data
    }

    pub fn gathered_length(&self) -> usize {
        self.gathered_length
    }
}
