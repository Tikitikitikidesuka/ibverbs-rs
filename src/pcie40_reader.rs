use crate::pcie40_stream::PCIe40StreamGuard;
use crate::zero_copy_reader::{DataGuard, ZeroCopyRingBufferReader};

struct PCIe40Reader<'a> {
    stream_guard: PCIe40StreamGuard<'a>,
}

impl<'a> PCIe40Reader<'a> {
    pub fn new(stream_guard: PCIe40StreamGuard<'a>) -> Self {
        Self { stream_guard }
    }
}

impl<'a> ZeroCopyRingBufferReader for PCIe40Reader<'a> {
    fn data(&self) -> DataGuard<Self> {
        todo!()
        //DataGuard::new(self, )
    }

    fn load_data(&mut self, num_bytes: usize) -> usize {
        todo!()
    }

    fn load_all_data(&self) -> usize {
        todo!()
    }

    fn discard_data(&mut self, num_bytes: usize) -> usize {
        todo!()
    }
}