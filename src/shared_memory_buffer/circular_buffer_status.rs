use libc::c_int;

#[repr(C, packed)]
pub struct CircularBufferStatus {
    write_status: u64,
    read_status: u64,
    size: usize,
    alignment_2pow: usize,
    id: c_int,
}

const PTR_MASK: u64 = 0x7FFFFFFFFFFFFFFF;
const WRAP_MASK: u64 = !PTR_MASK;

impl CircularBufferStatus {
    pub fn new(size: usize, alignment_2pow: usize, id: c_int) -> Self {
        Self {
            write_status: 0,
            read_status: 0,
            size,
            alignment_2pow,
            id,
        }
    }

    pub fn is_empty(&self) -> bool {
        // Buffer is empty when read and write positions are equal and on the same page
        //self.write_ptr() == self.read_ptr() && self.same_page()
        self.write_status == self.read_status
    }

    pub fn is_full(&self) -> bool {
        // Buffer is full when read and write positions are equal but not on the same page
        self.write_ptr() == self.read_ptr() && !self.same_page()
    }

    pub fn same_page(&self) -> bool {
        // Read and write pointers are on the same page when their wrap flags are the same
        (self.write_status & WRAP_MASK) == (self.read_status & WRAP_MASK)
    }

    pub fn write_ptr(&self) -> u64 {
        (self.write_status & PTR_MASK) << 1
    }

    pub fn read_ptr(&self) -> u64 {
        (self.read_status & PTR_MASK) << 1
    }

    pub fn set_write_ptr(&mut self, write_ptr: u64) {
        self.write_status = (self.write_status & WRAP_MASK) | ((write_ptr >> 1) & PTR_MASK);
    }

    pub fn set_read_ptr(&mut self, read_ptr: u64) {
        self.read_status = (self.read_status & PTR_MASK) | ((read_ptr >> 1) & PTR_MASK);
    }

    pub fn toggle_write_wrap(&mut self) {
        self.write_status = self.write_status ^ WRAP_MASK;
    }

    pub fn toggle_read_wrap(&mut self) {
        self.read_status = self.write_status ^ WRAP_MASK;
    }

    pub fn tail_free_space(&self) -> usize {
        if self.same_page() {
            self.size - self.write_ptr() as usize
        } else {
            (self.read_ptr() - self.write_ptr()) as usize
        }
    }

    pub fn head_free_space(&self) -> usize {
        if self.same_page() {
            self.read_ptr() as usize
        } else {
            0
        }
    }
}

impl Default for CircularBufferStatus {
    fn default() -> Self {
        Self::new()
    }
}
