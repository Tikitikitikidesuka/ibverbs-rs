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

#[derive(Clone, Copy)]
pub struct PtrStatus {
    status: u64,
}

impl PtrStatus {
    pub fn new(status: u64) -> Self {
        Self { status }
    }

    pub fn ptr(&self) -> u64 {
        (self.status & PTR_MASK) << 1
    }

    pub fn set_ptr(&mut self, ptr: u64) {
        self.status = (self.status & WRAP_MASK) | ((ptr >> 1) & PTR_MASK);
    }

    pub fn wrap(&self) -> bool {
        (self.status & WRAP_MASK) != 0
    }

    pub fn set_wrap(&mut self, wrap: bool) {
        if wrap {
            self.status |= WRAP_MASK;
        } else {
            self.status &= !WRAP_MASK;
        }
    }

    pub fn toggle_wrap(&mut self) {
        self.status ^= WRAP_MASK;
    }

    pub fn add(self, offset: usize, buffer_size: usize) -> Self {
        let current_ptr = self.ptr() as usize;
        let total_distance = current_ptr + offset;

        // Calculate how many times we wrapped
        let wrap_count = total_distance / buffer_size;
        let final_position = total_distance % buffer_size;

        // Calculate new wrap bit state
        let current_wrap = self.wrap();
        let new_wrap = current_wrap ^ (wrap_count % 2 == 1);

        // Construct new status
        let mut new_status = (final_position as u64) >> 1;
        if new_wrap {
            new_status |= WRAP_MASK;
        }

        Self { status: new_status }
    }
    pub fn add_assign(&mut self, offset: usize, buffer_size: usize) {
        let current_ptr = self.ptr() as usize;
        let total_distance = current_ptr + offset;

        // Calculate how many times we wrapped
        let wrap_count = total_distance / buffer_size;
        let final_position = total_distance % buffer_size;

        // Toggle wrap bit if we wrapped an odd number of times
        if wrap_count % 2 == 1 {
            self.status ^= WRAP_MASK;
        }

        // Update pointer (shift right by 1 to store in status)
        self.status = (self.status & WRAP_MASK) | ((final_position as u64) >> 1);
    }
}

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

    pub fn buffer_is_empty(write_status: PtrStatus, read_status: PtrStatus) -> bool {
        // Buffer is empty when read and write positions are equal and on the same wrap
        write_status.ptr() == read_status.ptr() && write_status.wrap() == read_status.wrap()
    }

    pub fn is_empty(&self) -> bool {
        Self::buffer_is_empty(self.write_status(), self.read_status())
    }

    pub fn buffer_is_full(write_status: PtrStatus, read_status: PtrStatus) -> bool {
        // Buffer is full when read and write positions are equal but not on the same wrap
        write_status.ptr() == read_status.ptr() && write_status.wrap() != read_status.wrap()
    }

    pub fn is_full(&self) -> bool {
        Self::buffer_is_full(self.write_status(), self.read_status())
    }

    pub fn same_page(&self) -> bool {
        // Read and write pointers are on the same page when their wrap flags are the same
        (self.write_status & WRAP_MASK) == (self.read_status & WRAP_MASK)
    }

    pub fn write_status(&self) -> PtrStatus {
        PtrStatus::new(self.write_status)
    }

    pub fn read_status(&self) -> PtrStatus {
        PtrStatus::new(self.read_status)
    }

    pub fn set_write_status(&mut self, new_status: PtrStatus) {
        self.write_status = new_status.status;
    }

    pub fn set_read_status(&mut self, new_status: PtrStatus) {
        self.read_status = new_status.status;
    }

    pub fn buffer_tail_free_space(write_status: PtrStatus, read_status: PtrStatus, buffer_size: usize) -> usize {
        if write_status.wrap() == read_status.wrap() {
            buffer_size - write_status.ptr() as usize
        } else {
            // Write has wrapped ahead, can only go up to read
            if read_status.ptr() > write_status.ptr() {
                (read_status.ptr() - write_status.ptr()) as usize
            } else {
                0  // Write caught up to read at position 0
            }
        }
    }

    pub fn buffer_head_free_space(write_status: PtrStatus, read_status: PtrStatus) -> usize {
        if write_status.wrap() == read_status.wrap() {
            read_status.ptr() as usize
        } else {
            0
        }
    }

    pub fn buffer_available_to_write(write_status: PtrStatus, read_status: PtrStatus, buffer_size: usize) -> usize {
        if write_status.wrap() == read_status.wrap() {
            // Write hasn't wrapped ahead of read
            if write_status.ptr() >= read_status.ptr() {
                // Can write to end and wrap around to read
                (buffer_size - write_status.ptr() as usize) + read_status.ptr() as usize
            } else {
                // This shouldn't happen on same wrap
                (read_status.ptr() - write_status.ptr()) as usize
            }
        } else {
            // Write has wrapped ahead of read
            // Can only write up to read position
            (read_status.ptr() - write_status.ptr()) as usize
        }
    }

    pub fn buffer_available_to_read(write_status: PtrStatus, read_status: PtrStatus, buffer_size: usize) -> usize {
        if write_status.wrap() == read_status.wrap() {
            // No wrap difference
            (write_status.ptr() - read_status.ptr()) as usize
        } else {
            // Write has wrapped, read hasn't
            (buffer_size - read_status.ptr() as usize) + write_status.ptr() as usize
        }
    }

    pub fn tail_free_space(&self) -> usize {
        let write_status = self.write_status();
        let read_status = self.read_status();
        Self::buffer_tail_free_space(write_status, read_status, self.size)
    }

    pub fn head_free_space(&self) -> usize {
        let write_status = self.write_status();
        let read_status = self.read_status();
        Self::buffer_head_free_space(write_status, read_status)
    }

    pub fn available_to_write(&self) -> usize {
        let write_status = self.write_status();
        let read_status = self.read_status();
        Self::buffer_available_to_write(write_status, read_status, self.size)
    }

    pub fn available_to_read(&self) -> usize {
        Self::buffer_available_to_read(self.write_status(), self.read_status(), self.size)
    }

    pub fn size(&self) -> usize {
        self.size
    }
}
