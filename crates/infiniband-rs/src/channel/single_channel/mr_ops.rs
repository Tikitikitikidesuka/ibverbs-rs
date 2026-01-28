use crate::channel::raw_channel::pending_work::WorkPollError;
use crate::channel::single_channel::SingleChannel;
use crate::ibverbs::memory_region::MemoryRegion;
use crate::ibverbs::remote_memory_region::RemoteMemoryRegion;
use std::io;
use std::time::Duration;

impl SingleChannel {
    pub fn register_local_mr(&mut self, memory: &[u8]) -> io::Result<MemoryRegion> {
        unsafe {
            self.pd
                .register_local_mr(memory.as_ptr() as *mut _, memory.len())
        }
    }

    /// # Safety
    /// This memory can be mutated at any point. It is the user's responsibility to enforce some
    /// sort of protocol to avoid breaking aliasing rules on its borrows.
    pub unsafe fn register_shared_mr(&mut self, memory: &[u8]) -> io::Result<MemoryRegion> {
        unsafe {
            self.pd
                .register_shared_mr(memory.as_ptr() as *mut _, memory.len())
        }
    }

    pub fn register_local_dmabuf_mr(
        &mut self,
        fd: i32,
        offset: u64,
        length: usize,
        iova: u64,
    ) -> io::Result<MemoryRegion> {
        unsafe { self.pd.register_local_dmabuf(fd, offset, length, iova) }
    }

    /// # Safety
    /// This memory can be mutated at any point. It is the user's responsibility to enforce some
    /// sort of protocol to avoid breaking aliasing rules on its borrows.
    pub unsafe fn register_shared_dmabuf_mr(
        &mut self,
        fd: i32,
        offset: u64,
        length: usize,
        iova: u64,
    ) -> io::Result<MemoryRegion> {
        unsafe { self.pd.register_shared_dmabuf(fd, offset, length, iova) }
    }

    pub fn share_mr(&mut self, mr: &MemoryRegion) -> io::Result<()> {
        self.channel
            .write(
                self.meta_mr
                    .prepare_write_remote_mr_wr(mr.remote())
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::ResourceBusy,
                            "Peer has not acknowledged a previous remote mr share request",
                        )
                    })?,
            )
            .map_err(|e| {
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

        Ok(())
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
}
