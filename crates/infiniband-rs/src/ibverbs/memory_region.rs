use crate::protection_domain::IbvProtectionDomainInner;
use ibverbs_sys::*;
use std::ffi::c_void;
use std::io;
use std::slice::from_raw_parts_mut;
use std::sync::Arc;

pub struct IbvMemoryRegion {
    pub(super) pd: Arc<IbvProtectionDomainInner>,
    pub(super) mr: *mut ibv_mr,
}

unsafe impl Sync for IbvMemoryRegion {}
unsafe impl Send for IbvMemoryRegion {}

impl Drop for IbvMemoryRegion {
    fn drop(&mut self) {
        let mr = self.mr;
        let debug_text = format!("{:?}", self);
        let errno = unsafe { ibv_dereg_mr(self.mr) };
        if errno != 0 {
            let e = io::Error::from_raw_os_error(errno);
            log::error!(
                "({debug_text}) -> Failed to deregister memory region with `ibv_dereg_mr({mr:p})`: {e}"
            );
        }
    }
}

impl std::fmt::Debug for IbvMemoryRegion {
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

impl IbvMemoryRegion {
    /// # Safety
    /// The user is responsible for ensuring the memory registered
    /// is not deallocated for as long as it is registered.
    pub(super) unsafe fn register_with_permissions(
        pd: Arc<IbvProtectionDomainInner>,
        memory: *mut [u8],
        access_flags: ibv_access_flags,
    ) -> io::Result<IbvMemoryRegion> {
        let mr = unsafe {
            ibv_reg_mr(
                pd.pd,
                memory as *mut u8 as *mut c_void,
                memory.len(),
                access_flags.0 as i32,
            )
        };
        if mr.is_null() {
            Err(io::Error::last_os_error())
        } else {
            Ok(IbvMemoryRegion { pd, mr })
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
        pd: Arc<IbvProtectionDomainInner>,
        fd: i32,
        offset: u64,
        len: usize,
        iova: u64,
        access_flags: ibv_access_flags,
    ) -> io::Result<IbvMemoryRegion> {
        let mr = unsafe { ibv_reg_dmabuf_mr(pd.pd, offset, len, iova, fd, access_flags.0 as i32) };

        if mr.is_null() {
            Err(io::Error::last_os_error())
        } else {
            Ok(IbvMemoryRegion { pd, mr })
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
