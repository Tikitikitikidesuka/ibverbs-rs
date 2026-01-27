use crate::channel::single_channel::SingleChannel;
use crate::ibverbs::memory_region::MemoryRegion;
use crate::ibverbs::queue_pair_builder::AccessFlags;
use crate::ibverbs::remote_memory_region::RemoteMemoryRegion;
use std::io;

impl SingleChannel {
    pub fn register_local_mr(&mut self, memory: &mut [u8]) -> io::Result<MemoryRegion> {
        unsafe {
            Ok(self
                .pd
                .register_local_mr(memory.as_mut_ptr(), memory.len())?)
        }
    }

    pub fn register_shared_mr(&mut self, memory: &mut [u8]) -> io::Result<MemoryRegion> {
        let mr = unsafe {
            self.pd
                .register_shared_mr(memory.as_mut_ptr(), memory.len())?
        };

        let remote_mr = mr.remote();

        // todo: handle instead of unwrap
        let wr = self.meta_mr.prepare_write_remote_mr_wr(remote_mr).unwrap();

        // todo: handle instead of unwrap
        self.channel.write(wr).unwrap();

        Ok(mr)
    }

    pub fn accept_remote_mr(&mut self) -> io::Result<RemoteMemoryRegion> {
        // todo: add timeout
        loop {
            if let Some(remote_mr) = self.meta_mr.read_remote_mr() {
                let wr = self.meta_mr.prepare_write_ack_remote_mr_wr().unwrap();
                self.channel.write(wr).unwrap();
                return Ok(remote_mr);
            }
        }
    }

    pub fn register_local_dmabuf_mr(
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
