use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

#[repr(C, packed)]
#[derive(Debug, Clone, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub struct Header {
    pub write_status: PtrStatus,
    pub read_status: PtrStatus,
    pub size: u64,
    pub alignment_pow2: u64,
    pub id: u32,
}

const PTR_MASK: u64 = 0x7FFFFFFFFFFFFFFF;
const WRAP_MASK: u64 = !PTR_MASK;

#[repr(transparent)]
#[derive(Debug, Copy, Clone, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub struct PtrStatus {
    status: u64,
}

impl PtrStatus {
    pub fn zero() -> Self {
        PtrStatus { status: 0 }
    }

    pub fn address(&self) -> u64 {
        (self.status & PTR_MASK) << 1
    }

    pub fn with_address(mut self, address: u64) -> Self {
        self.status = (self.status & WRAP_MASK) | ((address >> 1) & PTR_MASK);
        self
    }

    pub fn wrap_flag(&self) -> bool {
        (self.status & WRAP_MASK) != 0
    }

    pub fn set_wrap_flag(mut self) -> Self {
        self.status |= WRAP_MASK;
        self
    }

    pub fn reset_wrap_flag(mut self) -> Self {
        self.status &= !WRAP_MASK;
        self
    }

    pub fn toggle_wrap(mut self) -> Self {
        self.status ^= WRAP_MASK;
        self
    }

    pub fn wrapped_offset(self, offset: usize, buffer_size: usize) -> Self {
        let current_ptr = self.address() as usize;
        let total_distance = current_ptr + offset;

        // Calculate how many times we wrapped
        let wrap_count = total_distance / buffer_size;
        let final_position = total_distance % buffer_size;

        // Calculate new wrap flag
        let current_wrap = self.wrap_flag();
        let new_wrap = current_wrap ^ (wrap_count % 2 == 1);

        // Construct new status
        let mut new_status = (final_position as u64) >> 1;
        if new_wrap {
            new_status |= WRAP_MASK;
        }

        Self { status: new_status }
    }
}
