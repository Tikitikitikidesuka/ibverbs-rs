use crate::ibverbs::access_config::AccessFlags;
use crate::ibverbs::error::{IbvError, IbvResult};
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::ibverbs::remote_memory_region::RemoteMemoryRegion;
use crate::ibverbs::scatter_gather_element::*;
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
        log::debug!("MemoryRegion deregistered");
        let errno = unsafe { ibv_dereg_mr(self.mr) };
        if errno != 0 {
            let error = IbvError::from_errno_with_msg(errno, "Failed to deregister memory region");
            log::error!("{error}");
        }
    }
}

impl std::fmt::Debug for MemoryRegion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryRegion")
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
    ) -> IbvResult<MemoryRegion> {
        let mr = unsafe {
            ibv_reg_mr(
                pd.inner.pd,
                address as *mut c_void,
                length,
                access_flags.code() as i32,
            )
        };
        if mr.is_null() {
            Err(IbvError::from_errno_with_msg(
                io::Error::last_os_error().raw_os_error().unwrap(),
                "Failed to register memory region",
            ))
        } else {
            log::debug!("MemoryRegion registered");
            Ok(MemoryRegion { pd: pd.clone(), mr })
        }
    }

    pub fn register_local_mr(
        pd: &ProtectionDomain,
        address: *mut u8,
        length: usize,
    ) -> IbvResult<MemoryRegion> {
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
    ) -> IbvResult<MemoryRegion> {
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
    ) -> IbvResult<MemoryRegion> {
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
            Err(IbvError::from_errno_with_msg(
                io::Error::last_os_error().raw_os_error().unwrap(),
                "Failed to register memory region",
            ))
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
    ) -> IbvResult<MemoryRegion> {
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
    ) -> IbvResult<MemoryRegion> {
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
    pub fn gather_element<'a>(
        &'a self,
        data: &'a [u8],
    ) -> Result<GatherElement<'a>, ScatterGatherElementError> {
        GatherElement::<'a>::new(self, data)
    }

    pub fn gather_element_unchecked<'a>(&'a self, data: &'a [u8]) -> GatherElement<'a> {
        GatherElement::<'a>::new_unchecked(self, data)
    }

    pub fn scatter_element<'a>(
        &'a self,
        data: &'a mut [u8],
    ) -> Result<ScatterElement<'a>, ScatterGatherElementError> {
        ScatterElement::<'a>::new(self, data)
    }

    pub fn scatter_element_unchecked<'a>(&'a self, data: &'a mut [u8]) -> ScatterElement<'a> {
        ScatterElement::<'a>::new_unchecked(self, data)
    }

    pub fn encloses(&self, address: *const u8, length: usize) -> bool {
        let mr_start = self.address() as usize;
        let mr_end = mr_start + self.length();
        let data_start = address as usize;
        let data_end = data_start + length;
        data_start >= mr_start && data_end <= mr_end
    }

    pub fn encloses_slice(&self, slice: &[u8]) -> bool {
        self.encloses(slice.as_ptr(), slice.len())
    }
}
