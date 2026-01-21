use crate::ibverbs::protection_domain::ProtectionDomainInner;
use crate::ibverbs::scatter_gather_element::{
    GatherElement, ScatterElement, ScatterGatherElementError,
};
use ibverbs_sys::*;
use std::ffi::c_void;
use std::io;
use std::sync::Arc;

pub struct MemoryRegion {
    pd: Arc<ProtectionDomainInner>,
    mr: *mut ibv_mr,
}

unsafe impl Sync for MemoryRegion {}
unsafe impl Send for MemoryRegion {}

impl Drop for MemoryRegion {
    fn drop(&mut self) {
        log::debug!("IbvMemoryRegion deregistered");
        let mr = self.mr;
        let errno = unsafe { ibv_dereg_mr(self.mr) };
        if errno != 0 {
            let debug_text = format!("{:?}", self);
            let e = io::Error::from_raw_os_error(errno);
            log::error!(
                "({debug_text}) -> Failed to deregister memory region with `ibv_dereg_mr({mr:p})`: {e}"
            );
        }
    }
}

impl std::fmt::Debug for MemoryRegion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IbvMemoryRegion")
            .field("address", &(unsafe { (*self.mr).addr }))
            .field("length", &(unsafe { (*self.mr).length }))
            .field("handle", &(unsafe { (*self.mr).handle }))
            .field("lkey", &(unsafe { (*self.mr).lkey }))
            .field("rkey", &(unsafe { (*self.mr).rkey }))
            .field("pd", &self.pd)
            .finish()
    }
}

impl MemoryRegion {
    /// # Safety
    /// The user is responsible for ensuring the memory registered
    /// is not deallocated for as long as it is registered.
    pub(super) unsafe fn register_with_permissions(
        pd: Arc<ProtectionDomainInner>,
        address: *mut u8,
        length: usize,
        access_flags: ibv_access_flags,
    ) -> io::Result<MemoryRegion> {
        let mr =
            unsafe { ibv_reg_mr(pd.pd, address as *mut c_void, length, access_flags.0 as i32) };
        if mr.is_null() {
            Err(io::Error::last_os_error())
        } else {
            log::debug!("IbvMemoryRegion registered");
            Ok(MemoryRegion { pd, mr })
        }
    }

    /// Registers a DMA-BUF with the given access flags.
    ///
    /// # Arguments
    /// * `fd` - The file descriptor of the DMA-BUF to be registered.
    /// * `offset`, `len` - The MR starts at `offset` of the dma-buf and its size is `len`.
    /// * `iova` - The argument iova specifies the virtual base address of the MR when accessed through a lkey or rkey.
    ///   Note: `iova` must have the same page offset as `offset`
    ///
    /// # Safety
    /// The DMA-BUF and its mapped memory must not be deallocated while they remain registered.
    pub(super) unsafe fn register_dmabuf(
        pd: Arc<ProtectionDomainInner>,
        fd: i32,
        offset: u64,
        len: usize,
        iova: u64,
        access_flags: ibv_access_flags,
    ) -> io::Result<MemoryRegion> {
        let mr = unsafe { ibv_reg_dmabuf_mr(pd.pd, offset, len, iova, fd, access_flags.0 as i32) };

        if mr.is_null() {
            Err(io::Error::last_os_error())
        } else {
            log::debug!("IbvMemoryRegion registered");
            Ok(MemoryRegion { pd, mr })
        }
    }

    pub fn lkey(&self) -> u32 {
        unsafe { *self.mr }.lkey
    }

    pub fn rkey(&self) -> u32 {
        unsafe { *self.mr }.rkey
    }

    pub fn address(&self) -> *mut u8 {
        unsafe { (*self.mr).addr as *mut u8 }
    }

    pub fn length(&self) -> usize {
        unsafe { (*self.mr).length }
    }
}

impl MemoryRegion {
    pub fn prepare_scatter_element<'a>(
        &self,
        data: &'a [u8],
    ) -> Result<ScatterElement<'a>, ScatterGatherElementError> {
        ScatterElement::<'a>::new(self, data)
    }

    pub fn prepare_gather_element<'a>(
        &self,
        data: &'a mut [u8],
    ) -> Result<GatherElement<'a>, ScatterGatherElementError> {
        GatherElement::<'a>::new(self, data)
    }

    pub fn encloses(&self, slice: &[u8]) -> bool {
        let mr_start = self.address() as usize;
        let mr_end = mr_start + self.length();
        let data_start = slice.as_ptr() as usize;
        let data_end = data_start + slice.len();
        data_start >= mr_start && data_end <= mr_end
    }
}
