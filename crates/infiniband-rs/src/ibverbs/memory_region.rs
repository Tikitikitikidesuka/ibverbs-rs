use crate::ibverbs::access_config::AccessFlags;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::ibverbs::remote_memory_region::RemoteMemoryRegion;
use crate::ibverbs::scatter_gather_element::{
    GatherElement, ScatterElement, ScatterGatherElementError,
};
use ibverbs_sys::*;
use std::ffi::c_void;
use std::io;

pub struct MemoryRegion {
    pd: ProtectionDomain,
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
    /// If the memory region registered has remote write access the memory can be DMA aliased mutably
    /// by remote peers. It can change at any point so Rust aliasing rules on the memory must be enforced
    /// manually by the user.
    /// It is also safe to pass a non owned memory region, it gets detected by the hardware
    /// and returned as an error.
    pub unsafe fn register_with_permissions(
        pd: &ProtectionDomain,
        address: *mut u8,
        length: usize,
        access_flags: AccessFlags,
    ) -> io::Result<MemoryRegion> {
        let mr = unsafe {
            ibv_reg_mr(
                pd.inner.pd,
                address as *mut c_void,
                length,
                access_flags.code() as i32,
            )
        };
        if mr.is_null() {
            Err(io::Error::last_os_error())
        } else {
            log::debug!("IbvMemoryRegion registered");
            Ok(MemoryRegion { pd: pd.clone(), mr })
        }
    }

    pub fn register_local_mr(
        pd: &ProtectionDomain,
        address: *mut u8,
        length: usize,
    ) -> io::Result<MemoryRegion> {
        unsafe {
            Self::register_with_permissions(
                pd,
                address,
                length,
                AccessFlags::new().with_local_write(),
            )
        }
    }

    pub unsafe fn register_shared_mr(
        pd: &ProtectionDomain,
        address: *mut u8,
        length: usize,
    ) -> io::Result<MemoryRegion> {
        unsafe {
            Self::register_with_permissions(
                pd,
                address,
                length,
                AccessFlags::new()
                    .with_local_write()
                    .with_remote_read()
                    .with_remote_write(),
            )
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
    /// If the memory region registered has remote write access the memory can be DMA aliased mutably
    /// by remote peers. It can change at any point so Rust aliasing rules on the memory must be enforced
    /// manually by the user.
    pub unsafe fn register_dmabuf(
        pd: &ProtectionDomain,
        fd: i32,
        offset: u64,
        length: usize,
        iova: u64,
        access_flags: AccessFlags,
    ) -> io::Result<MemoryRegion> {
        let mr = unsafe {
            ibv_reg_dmabuf_mr(
                pd.inner.pd,
                offset,
                length,
                iova,
                fd,
                access_flags.code() as i32,
            )
        };

        if mr.is_null() {
            Err(io::Error::last_os_error())
        } else {
            log::debug!("IbvMemoryRegion registered");
            Ok(MemoryRegion { pd: pd.clone(), mr })
        }
    }

    pub fn register_local_dmabuf(
        pd: &ProtectionDomain,
        fd: i32,
        offset: u64,
        length: usize,
        iova: u64,
    ) -> io::Result<MemoryRegion> {
        unsafe {
            Self::register_dmabuf(
                pd,
                fd,
                offset,
                length,
                iova,
                AccessFlags::new().with_local_write(),
            )
        }
    }

    pub unsafe fn register_shared_dmabuf(
        pd: &ProtectionDomain,
        fd: i32,
        offset: u64,
        length: usize,
        iova: u64,
    ) -> io::Result<MemoryRegion> {
        unsafe {
            Self::register_dmabuf(
                pd,
                fd,
                offset,
                length,
                iova,
                AccessFlags::new()
                    .with_local_write()
                    .with_remote_read()
                    .with_remote_write(),
            )
        }
    }
}

impl MemoryRegion {
    pub fn rkey(&self) -> u32 {
        unsafe { *self.mr }.rkey
    }

    pub fn address(&self) -> u64 {
        unsafe { (*self.mr).addr as u64 }
    }

    pub fn length(&self) -> usize {
        unsafe { (*self.mr).length }
    }

    pub fn lkey(&self) -> u32 {
        unsafe { *self.mr }.lkey
    }

    pub fn remote(&self) -> RemoteMemoryRegion {
        RemoteMemoryRegion::new(self.address(), self.length(), self.rkey())
    }
}

impl MemoryRegion {
    pub fn prepare_gather_element<'a>(
        &'a self,
        data: &'a [u8],
    ) -> Result<GatherElement<'a>, ScatterGatherElementError> {
        GatherElement::<'a>::new(self, data)
    }

    pub fn prepare_scatter_element<'a>(
        &'a self,
        data: &'a mut [u8],
    ) -> Result<ScatterElement<'a>, ScatterGatherElementError> {
        ScatterElement::<'a>::new(self, data)
    }

    pub fn encloses(&self, slice: &[u8]) -> bool {
        let mr_start = self.address() as usize;
        let mr_end = mr_start + self.length();
        let data_start = slice.as_ptr() as usize;
        let data_end = data_start + slice.len();
        data_start >= mr_start && data_end <= mr_end
    }
}
