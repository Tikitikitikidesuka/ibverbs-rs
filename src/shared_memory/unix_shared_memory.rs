use std::ffi::CString;
use std::io::Error;
use std::os::fd::RawFd;
use thiserror::Error;

struct SharedMemory {
    fd: RawFd,
    size: usize,
    name: String,
}

#[derive(Debug, Error)]
enum SharedMemoryOpenError {
    #[error("System error: {0}")]
    SystemError(#[from] std::io::Error),

    #[error("Invalid shared memory segment: {segment_name}")]
    InvalidSegment { segment_name: String },
}

impl SharedMemory {
    fn open<T: Into<String>>(name: T) -> Result<Self, SharedMemoryOpenError> {
        let name = name.into();

        let c_name =
            CString::new(name.as_str()).map_err(|_| SharedMemoryOpenError::InvalidSegment {
                segment_name: name.clone(),
            })?;

        let fd = unsafe { libc::shm_open(c_name.as_ptr(), libc::O_RDWR, 0) };
        if fd < 0 {
            Err(SharedMemoryOpenError::SystemError(Error::last_os_error()))?;
        }

        // Get size
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstat(fd, &mut stat) } != 0 {
            let err = Error::last_os_error();
            unsafe { libc::close(fd) };
            Err(SharedMemoryOpenError::SystemError(err))?;
        }

        let size = stat.st_size as usize;

        Ok(Self { fd, size, name })
    }

    fn create<T: Into<String>>(
        name: T,
        size: usize,
    ) -> Result<SharedMemory, SharedMemoryOpenError> {
        let name = name.into();

        let c_name =
            CString::new(name.as_str()).map_err(|_| SharedMemoryOpenError::InvalidSegment {
                segment_name: name.clone(),
            })?;

        let fd = unsafe { libc::shm_open(c_name.as_ptr(), libc::O_CREAT | libc::O_RDWR, 0o666) };
        if fd < 0 {
            Err(SharedMemoryOpenError::SystemError(Error::last_os_error()))?;
        }

        // Set size
        if unsafe { libc::ftruncate(fd, size as libc::off_t) } != 0 {
            let err = Error::last_os_error();
            unsafe { libc::close(fd) };
            Err(SharedMemoryOpenError::SystemError(err))?;
        }

        Ok(Self { fd, size, name })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        SharedMemory::open("test").unwrap();
    }
}
