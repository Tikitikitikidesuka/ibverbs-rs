use nix::sys::stat::{Mode, umask};
use std::ffi::CString;
use std::ops::Not;
use std::os::fd::RawFd;
use nix::sys::stat;
use thiserror::Error;

pub struct SharedMemoryEndpoint {
    file_descriptor: RawFd,
    name: String,
    size: usize,
}

pub struct SharedMemory<'a> {
    endpoint: &'a SharedMemoryEndpoint,
    mapped_address: *mut libc::c_void,
    size: usize,
}

#[derive(Debug, Error)]
pub enum SharedMemoryError {
    #[error("System error: {0}")]
    SystemError(#[from] std::io::Error),

    #[error("Invalid shared memory segment name: {segment_name}")]
    InvalidSegmentName { segment_name: String },

    #[error("Unable to open shared memory: {0}")]
    OpenError(#[from] std::io::Error),

    #[error("Unable to create shared memory: {0}")]
    CreateError(#[from] std::io::Error),

    #[error("Unable to read shared memory: {0}")]
    ReadError(#[from] std::io::Error),

    #[error("Unable to write to shared memory: {0}")]
    WriteError(#[from] std::io::Error),

    #[error("Unable to map shared memory: {0}")]
    MapError(#[from] std::io::Error),
}

impl SharedMemoryEndpoint {
    pub fn open<T: Into<String>>(name: T) -> Result<SharedMemoryEndpoint, SharedMemoryError> {
        let name = name.into();

        let c_name = CString::new(&name).map_err(|_| SharedMemoryError::InvalidSegmentName {
            segment_name: name.clone(),
        })?;

        // Open shared memory
        let file_descriptor = unsafe { libc::shm_open(c_name.as_ptr(), libc::O_RDWR, 0) };
        if file_descriptor < 0 {
            Err(SharedMemoryError::OpenError(std::io::Error::last_os_error()))?;
        }

        // Get shared memory size
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstat(file_descriptor, &mut stat) } != 0 {
            let err = std::io::Error::last_os_error();
            unsafe { libc::close(file_descriptor) };
            Err(SharedMemoryError::ReadError(err))?;
        }

        let size = stat.st_size as usize;

        Ok(SharedMemoryEndpoint {
            file_descriptor,
            size,
            name,
        })
    }

    pub fn create<T: Into<String>>(name: T, size: usize, permission_mode: Mode) -> Result<SharedMemoryEndpoint, SharedMemoryError> {
        let name = name.into();

        let c_name = CString::new(&name).map_err(|_| SharedMemoryError::InvalidSegmentName {
            segment_name: name.clone(),
        })?;

        // Create shared memory
        let old_mask = umask(permission_mode.not());
        let file_descriptor = unsafe {
            libc::shm_open(
                c_name.as_ptr(),
                libc::O_CREAT | libc::O_RDWR,
                permission_mode.bits(),
            )
        };
        umask(old_mask);

        if file_descriptor < 0 {
            Err(SharedMemoryError::CreateError(
                std::io::Error::last_os_error(),
            ))?;
        }

        // Set shared memory size
        if unsafe { libc::ftruncate(file_descriptor, size as i64) } < 0 {
            let err = std::io::Error::last_os_error();
            unsafe { libc::close(file_descriptor) };
            Err(SharedMemoryError::WriteError(err))?;
        }

        Ok(SharedMemoryEndpoint {
            file_descriptor,
            size,
            name,
        })
    }

    pub fn map(&self) -> Result<(), SharedMemoryError> {
        let mapped_address = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                self.size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                self.file_descriptor,
                0
            )
        };

        if mapped_address == libc::MAP_FAILED {
            let err = std::io::Error::last_os_error();
            Err(SharedMemoryError::SystemError(err))?;
        }

        todo!()
    }
}
