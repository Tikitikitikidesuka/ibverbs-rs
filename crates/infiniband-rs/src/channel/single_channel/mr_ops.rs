use crate::channel::raw_channel::pending_work::WorkPollError;
use crate::channel::single_channel::SingleChannel;
use crate::ibverbs::memory_region::MemoryRegion;
use crate::ibverbs::remote_memory_region::RemoteMemoryRegion;
use std::io;
use std::time::Duration;

impl SingleChannel {
    pub fn register_local_mr(&mut self, memory: &mut [u8]) -> io::Result<MemoryRegion> {
        self.pd.register_local_mr(memory.as_mut_ptr(), memory.len())
    }

    pub fn register_local_mr_from_ptr(
        &mut self,
        address: *mut u8,
        length: usize,
    ) -> io::Result<MemoryRegion> {
        self.pd.register_local_mr(address, length)
    }

    /// # Safety
    /// This memory can be mutated at any point. It is the user's responsibility to enforce some
    /// sort of protocol to avoid breaking aliasing rules on its borrows.
    pub unsafe fn register_shared_mr(&mut self, memory: &mut [u8]) -> io::Result<MemoryRegion> {
        unsafe {
            self.pd
                .register_shared_mr(memory.as_ptr() as *mut _, memory.len())
        }
    }

    /// # Safety
    /// This memory can be mutated at any point. It is the user's responsibility to enforce some
    /// sort of protocol to avoid breaking aliasing rules on its borrows.
    pub unsafe fn register_shared_mr_from_ptr(
        &mut self,
        address: *mut u8,
        length: usize,
    ) -> io::Result<MemoryRegion> {
        unsafe { self.pd.register_shared_mr(address, length) }
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
        self.meta_mr.share_memory_region(&mut self.channel, mr)
    }

    pub fn accept_remote_mr(&mut self, timeout: Duration) -> io::Result<RemoteMemoryRegion> {
        self.meta_mr
            .accept_memory_region(&mut self.channel, timeout)
    }
}
