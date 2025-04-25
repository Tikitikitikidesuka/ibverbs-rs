use crate::shared_memory_buffer::circular_buffer_status::CircularBufferStatus;
use crate::utils;
use nix::sys::stat::{Mode, umask};
use std::ffi::CString;
use std::ops::Not;
use std::os::unix::fs::OpenOptionsExt;
use thiserror::Error;
use crate::shared_memory_buffer::shared_memory::{SharedMemoryEndpoint, SharedMemoryError};

const PERMISSION_MODE: Mode =
    Mode::S_IRUSR | Mode::S_IWUSR | Mode::S_IRGRP | Mode::S_IWGRP | Mode::S_IROTH | Mode::S_IWOTH;

struct SharedMemoryBuffer {
    shared_memory: SharedMemoryEndpoint,
}

#[derive(Debug, Error)]
enum SharedMemoryBufferError {
    #[error("Shared memory error: {0}")]
    SharedMemoryError(#[from] SharedMemoryError),

    #[error("Lock error: {0}")]
    LockError(#[from] LockError),

    #[error("Buffer configuration error: {0}")]
    ConfigurationError(String),
}

#[derive(Debug, Error)]
enum LockError {
    #[error("System error: {0}")]
    SystemError(#[from] std::io::Error),
}

impl SharedMemoryBuffer {
    pub fn create_reader_lock(&self) -> Result<std::fs::File, LockError> {
        Self::create_lock(self.reader_lock_path())
    }

    pub fn create_writer_lock(&self) -> Result<std::fs::File, LockError> {
        Self::create_lock(self.writer_lock_path())
    }

    pub fn initialize_buffer(
        &self,
        size: usize,
        alignment_2pow: u8,
        id: i32,
    ) -> Result<SharedMemoryBuffer, SharedMemoryBufferError> {
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as usize;
        let alignment = 1 << alignment_2pow;

        if alignment > page_size {
            Err(SharedMemoryBufferError::ConfigurationError(format!(
                "The alignment cannot be bigger than the page size (Alignment: {alignment}, Page size: {page_size})"
            )))?;
        }

        let buffer_status = CircularBufferStatus::new(size, alignment_2pow as usize, id);
        let buffer_start = utils::align_up_2pow(size_of::<CircularBufferStatus>(), alignment_2pow);
    }

    pub fn map_shared_memory() {
        todo!()
    }

    pub fn close_shared_memory() {
        todo!()
    }

    pub fn destroy_shared_memory() {
        todo!()
    }
}

impl SharedMemoryBuffer {
    fn create_lock<P: AsRef<std::fs::Path>>(lock_path: P) -> Result<std::fs::File, LockError> {
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .mode(PERMISSION_MODE.bits() as u32)
            .open(lock_path)
            .map_err(|e| LockError::SystemError(e))
    }

    fn reader_lock_path(&self) -> String {
        format!("/tmp/{}_reader.lock", self.shmem_name)
    }

    fn writer_lock_path(&self) -> String {
        format!("/tmp/{}_writer.lock", self.shmem_name)
    }
}
