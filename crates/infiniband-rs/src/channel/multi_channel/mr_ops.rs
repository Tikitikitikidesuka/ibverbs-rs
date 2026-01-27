use crate::channel::multi_channel::MultiChannel;
use crate::channel::raw_channel::pending_work::WorkPollError;
use crate::channel::raw_channel::polling_scope::PollingScope;
use crate::ibverbs::memory_region::MemoryRegion;
use crate::ibverbs::queue_pair_builder::AccessFlags;
use crate::ibverbs::remote_memory_region::RemoteMemoryRegion;
use std::io;
use std::time::Duration;

impl MultiChannel {
    pub fn register_local_mr(&mut self, memory: &mut [u8]) -> io::Result<MemoryRegion> {
        let mr = unsafe {
            self.pd
                .register_local_mr(memory.as_mut_ptr(), memory.len())?
        };

        Ok(mr)
    }

    pub fn register_shared_mr(&mut self, memory: &mut [u8]) -> io::Result<MemoryRegion> {
        let mr = unsafe {
            self.pd
                .register_shared_mr(memory.as_mut_ptr(), memory.len())?
        };

        let remote_mr = mr.remote();

        let MultiChannel {
            channels, meta_mrs, ..
        } = self;
        PollingScope::run(channels, |s| {
            s.channel_post_write(
                |s| Ok(&mut s[0]),
                meta_mrs[0].prepare_write_remote_mr_wr(remote_mr).unwrap(),
            )
            .unwrap();
        })
        .unwrap();

        Ok(mr)
    }

    pub fn share_mr(&mut self, memory: &mut [u8]) -> io::Result<MemoryRegion> {

    }

    pub fn accept_remote_mr(&mut self, timeout: Duration) -> io::Result<RemoteMemoryRegion> {
        let start = std::time::Instant::now();

        loop {
            if let Some(remote_mr) = self.meta_mr.read_remote_mr() {
                let wr = self.meta_mr.prepare_write_ack_remote_mr_wr().expect(
                    "Invariant violation: Failed to prepare ACK immediately after receiving new MR",
                );

                self.channel.write(wr).map_err(|e| {
                    match e {
                        WorkPollError::PollError(io_error) => io_error,
                        // This means the `prepare_write_remote_mr_wr` logic generated an invalid request.
                        WorkPollError::WorkError(work_error) => {
                            panic!(
                                "Invariant violation: Constructed invalid RDMA Work Request: {:?}",
                                work_error
                            )
                        }
                    }
                })?;

                return Ok(remote_mr);
            }

            if start.elapsed() > timeout {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Timed out waiting for peer to accept remote MR",
                ));
            }

            std::hint::spin_loop();
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
