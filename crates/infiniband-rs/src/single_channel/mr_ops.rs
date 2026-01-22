use crate::ibverbs::memory_region::MemoryRegion;
use crate::ibverbs::queue_pair_builder::AccessFlags;
use crate::single_channel::SingleChannel;
use std::io;

impl SingleChannel {
    pub fn register_mr(&mut self, memory: &mut [u8]) -> io::Result<MemoryRegion> {
        let mr = unsafe {
            self.pd.register_mr_with_permissions(
                memory.as_mut_ptr(),
                memory.len(),
                // TODO: Start with only local_write and add remote_read and remote_write when shared
                AccessFlags::new()
                    .with_local_write()
                    //.with_remote_read()
                    //.with_remote_write()
                    .into(),
            )?
        };

        Ok(mr)
    }

    pub fn register_dmabuf_mr(
        &mut self,
        fd: i32,
        offset: u64,
        length: usize,
        iova: u64,
    ) -> io::Result<MemoryRegion> {
        let mr = unsafe {
            self.pd.register_dmabuf(
                fd,
                offset,
                length,
                iova,
                // TODO: Start with only local_write and add remote_read and remote_write when shared
                AccessFlags::new()
                    .with_local_write()
                    //.with_remote_read()
                    //.with_remote_write()
                    .into(),
            )?
        };

        Ok(mr)
    }
}
