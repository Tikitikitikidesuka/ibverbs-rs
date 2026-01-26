#[derive(Copy, Clone, Debug)]
pub struct WorkSuccess {
    imm_data: Option<u32>,
    gathered_length: usize,
}

impl WorkSuccess {
    pub(super) fn new(imm_data: Option<u32>, gathered_length: usize) -> Self {
        Self {
            imm_data,
            gathered_length,
        }
    }
}

impl WorkSuccess {
    pub fn immediate_data(&self) -> Option<u32> {
        self.imm_data.map(|imm_data| u32::from_be(imm_data))
    }

    pub fn gathered_length(&self) -> usize {
        self.gathered_length
    }
}
