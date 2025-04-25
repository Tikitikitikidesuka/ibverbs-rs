#[repr(C, packed)]
struct CircularBufferStatus {
    write_status: u64,
    read_status: u64,
    size: usize,
    align: usize,
    id: i32,
}

const PTR_MASK: u64 = 0x7FFFFFFFFFFFFFFF;
const WRAP_MASK: u64 = !PTR_MASK;

impl CircularBufferStatus {
    pub fn new() -> Self {
        Self {
            write_status: 0,
            read_status: 0,
            size: 0,
            align: 0,
            id: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        // Buffer is empty when read and write positions are equal and on the same page
        self.write_ptr() == self.read_ptr() && self.same_page()
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
        self.write_status & PTR_MASK
    }

    pub fn read_ptr(&self) -> u64 {
        self.read_status & PTR_MASK
    }

    pub fn set_write_ptr(&mut self, new_ptr: u64) {

    }

    pub fn tail_free_space(&self) -> usize {
        if self.same_page() {
            self.size - self.write_ptr() as usize
        } else {
            (self.read_ptr()  - self.write_ptr()) as usize
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
