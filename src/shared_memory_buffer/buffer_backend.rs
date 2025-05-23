use crate::shared_memory_buffer::buffer_status::{CircularBufferStatus, PtrStatus};
use crate::shared_memory_buffer::file_lock::FileLock;
use crate::shared_memory_buffer::shared_memory::{MappedSharedMemory, SharedMemory};
use crate::utils;
use nix::sys::stat::Mode;
use std::path::{Path, PathBuf};
use std::ptr::{slice_from_raw_parts, slice_from_raw_parts_mut};
use thiserror::Error;

const PERMISSION_MODE: Mode = Mode::from_bits_truncate(0o666);

#[derive(Debug, Error)]
pub enum SharedMemoryBufferNewError {
    #[error("Unable to acquire shared memory at \"{shared_memory_path}\"")]
    UnableToAcquireSharedMemory { shared_memory_path: PathBuf },

    #[error("Unable to map shared memory at \"{shared_memory_path}\"")]
    UnableToMapSharedMemory { shared_memory_path: PathBuf },

    #[error("Unable to lock shared memory with file \"{read_lock_path}\"")]
    UnableToLockSharedMemory { read_lock_path: PathBuf },

    #[error(
        "Not enough memory to hold the buffer: Found {size} bytes, minimum is {minimum_size} bytes"
    )]
    MemoryNotBigEnough { size: usize, minimum_size: usize },
}

pub struct SharedMemoryBuffer {
    name: String,
    shared_memory: MappedSharedMemory,
    file_lock: FileLock,
    size: usize,
    alignment_2pow: u8,
    buffer_ptr: usize,
}

pub struct SharedMemoryReadBuffer {
    shmem_buffer: SharedMemoryBuffer,
}

pub struct SharedMemoryWriteBuffer {
    shmem_buffer: SharedMemoryBuffer,
}

unsafe impl Send for SharedMemoryBuffer {}
unsafe impl Sync for SharedMemoryBuffer {}

impl SharedMemoryBuffer {
    pub fn new_read_buffer<T: Into<String>>(
        name: T,
        alignment_2pow: u8,
    ) -> Result<SharedMemoryReadBuffer, SharedMemoryBufferNewError> {
        let name = name.into();

        // Lock the reader
        let mut file_lock = Self::get_reader_lock(&name)?;
        Self::try_lock(&mut file_lock)?;

        // Open and map shared memory
        let mut shared_memory = Self::open_mapped_shared_memory(&name)?;

        // Check it has enough data for the status
        if shared_memory.size() < size_of::<CircularBufferStatus>() {
            return Err(SharedMemoryBufferNewError::MemoryNotBigEnough {
                minimum_size: size_of::<CircularBufferStatus>(),
                size: shared_memory.size(),
            });
        }

        // Get buffer usable size from the header
        let size = unsafe {
            (&*(shared_memory.as_slice().as_ptr() as *const CircularBufferStatus)).size()
        };

        // Get buffer offset taking header and alignment into account
        let start_address = unsafe { shared_memory.as_slice().as_ptr() as usize };
        let buffer_ptr = utils::align_up_2pow(
            start_address + size_of::<CircularBufferStatus>(),
            alignment_2pow as u8,
        );

        // Create unsized buffer
        Ok(SharedMemoryReadBuffer {
            shmem_buffer: SharedMemoryBuffer {
                name: name.into(),
                shared_memory,
                file_lock,
                size,
                alignment_2pow,
                buffer_ptr,
            },
        })
    }

    pub fn new_write_buffer<T: Into<String>>(
        name: T,
        size: usize,
        alignment_2pow: u8,
    ) -> Result<SharedMemoryWriteBuffer, SharedMemoryBufferNewError> {
        let name = name.into();

        // Lock the writer
        let mut writer_lock = Self::get_writer_lock(&name)?;
        let mut reader_lock = Self::get_reader_lock(&name)?;
        Self::try_lock(&mut writer_lock)?;
        Self::try_lock(&mut reader_lock)?;

        // Delete any existing shared memory (in case of unclean shutdown)
        // Ignore errors as it might not exist
        let _ = SharedMemory::delete(&name);

        // Create and map shared memory
        let mut shared_memory = Self::create_mapped_shared_memory(&name, size, alignment_2pow)?;

        // Initialize the shared memory
        Self::initialize_shared_memory(&mut shared_memory, alignment_2pow, size)?;

        // Release reader lock when initialized
        Self::try_unlock(&mut reader_lock)?;

        // Get buffer offset taking header and alignment into account
        let start_address = unsafe { shared_memory.as_slice().as_ptr() as usize };
        let buffer_ptr = utils::align_up_2pow(
            start_address + size_of::<CircularBufferStatus>(),
            alignment_2pow as u8,
        );

        Ok(SharedMemoryWriteBuffer {
            shmem_buffer: SharedMemoryBuffer {
                name: name.into(),
                shared_memory,
                file_lock: writer_lock,
                size,
                alignment_2pow,
                buffer_ptr,
            },
        })
    }
}

impl Drop for SharedMemoryWriteBuffer {
    fn drop(&mut self) {
        // Unlink the shared memory - this removes it from the filesystem
        // Existing mappings remain valid until unmapped
        let _ = SharedMemory::delete(&self.shmem_buffer.name);

        // The writer lock will be released automatically when file_lock drops
        // The shared memory mapping will be unmapped when shared_memory drops
    }
}

impl SharedMemoryBuffer {
    pub unsafe fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.buffer_ptr as *const u8, self.size) }
    }

    pub unsafe fn as_slice_mut(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.buffer_ptr as *mut u8, self.size) }
    }

    pub fn read_status(&self) -> PtrStatus {
        self.status_ref().read_status()
    }

    pub fn write_status(&self) -> PtrStatus {
        self.status_ref().write_status()
    }

    pub fn set_read_status(&mut self, status: PtrStatus) {
        self.status_ref_mut().set_read_status(status);
    }

    pub fn set_write_status(&mut self, status: PtrStatus) {
        self.status_ref_mut().set_write_status(status);
    }

    fn status_ref(&self) -> &CircularBufferStatus {
        unsafe { &*(self.shared_memory.as_slice().as_ptr() as *const CircularBufferStatus) }
    }

    fn status_ref_mut(&mut self) -> &mut CircularBufferStatus {
        unsafe { &mut *(self.shared_memory.as_slice().as_ptr() as *mut _) }
    }
}

impl SharedMemoryReadBuffer {
    pub unsafe fn as_slice(&self) -> &[u8] {
        self.shmem_buffer.as_slice()
    }

    pub fn read_status(&self) -> PtrStatus {
        self.shmem_buffer.read_status()
    }

    pub fn size(&self) -> usize {
        self.shmem_buffer.size
    }

    pub fn write_status(&self) -> PtrStatus {
        self.shmem_buffer.write_status()
    }

    pub fn set_read_status(&mut self, status: PtrStatus) {
        self.shmem_buffer.set_read_status(status);
    }

    pub fn tail_free_space(&self) -> usize {
        self.shmem_buffer.status_ref().tail_free_space()
    }

    pub fn head_free_space(&self) -> usize {
        self.shmem_buffer.status_ref().head_free_space()
    }

    pub fn available_to_read(&self) -> usize {
        self.shmem_buffer.status_ref().available_to_read()
    }
}

impl SharedMemoryWriteBuffer {
    pub unsafe fn as_slice(&self) -> &[u8] {
        self.shmem_buffer.as_slice()
    }

    pub unsafe fn as_slice_mut(&mut self) -> &mut [u8] {
        self.shmem_buffer.as_slice_mut()
    }

    pub fn size(&self) -> usize {
        self.shmem_buffer.size
    }

    pub fn read_status(&self) -> PtrStatus {
        self.shmem_buffer.read_status()
    }

    pub fn write_status(&self) -> PtrStatus {
        self.shmem_buffer.write_status()
    }

    pub fn set_write_status(&mut self, status: PtrStatus) {
        self.shmem_buffer.set_write_status(status);
    }

    pub fn tail_free_space(&self) -> usize {
        self.shmem_buffer.status_ref().tail_free_space()
    }

    pub fn head_free_space(&self) -> usize {
        self.shmem_buffer.status_ref().head_free_space()
    }

    pub fn available_to_write(&self) -> usize {
        self.shmem_buffer.status_ref().available_to_write()
    }
}

//impl Drop for SharedMemoryBuffer {
// Files unlock automatically with their drop
// Shared memory unmaps and closes automatically with its drop
//}

impl SharedMemoryBuffer {
    fn initialize_shared_memory(
        shared_memory: &mut MappedSharedMemory,
        alignment_2pow: u8,
        size: usize,
    ) -> Result<(), SharedMemoryBufferNewError> {
        // Use the same alignment calculations as create_mapped_shared_memory
        let aligned_buffer_size = utils::align_up_2pow(size, alignment_2pow);

        // Create the initial status with the actual buffer size
        let initial_status = CircularBufferStatus::new(
            aligned_buffer_size,
            alignment_2pow as usize,
            0, // id - you might want to make this configurable
        );

        // Write the status header at the beginning of shared memory
        unsafe {
            let status_ptr = shared_memory.as_slice_mut().as_mut_ptr() as *mut CircularBufferStatus;
            std::ptr::write(status_ptr, initial_status);
        }

        Ok(())
    }

    fn open_mapped_shared_memory(
        name: impl Into<String>,
    ) -> Result<MappedSharedMemory, SharedMemoryBufferNewError> {
        let name = name.into();

        SharedMemory::open(&name)
            .map_err(|_| SharedMemoryBufferNewError::UnableToAcquireSharedMemory {
                shared_memory_path: name.clone().into(),
            })?
            .map()
            .map_err(|_| SharedMemoryBufferNewError::UnableToMapSharedMemory {
                shared_memory_path: name.into(),
            })
    }

    fn create_mapped_shared_memory(
        name: impl Into<String>,
        size: usize,
        alignment_2pow: u8, // Add this parameter
    ) -> Result<MappedSharedMemory, SharedMemoryBufferNewError> {
        let name = name.into();

        // Calculate total size needed (header + padding + buffer + padding)
        let aligned_header_size =
            utils::align_up_2pow(size_of::<CircularBufferStatus>(), alignment_2pow);
        let aligned_buffer_size = utils::align_up_2pow(size, alignment_2pow);
        let total_size = aligned_header_size + aligned_buffer_size;

        SharedMemory::create(&name, total_size, PERMISSION_MODE)
            .map_err(
                |_| SharedMemoryBufferNewError::UnableToAcquireSharedMemory {
                    shared_memory_path: name.clone().into(),
                },
            )?
            .map()
            .map_err(|_| SharedMemoryBufferNewError::UnableToMapSharedMemory {
                shared_memory_path: name.into(),
            })
    }

    fn get_reader_lock(name: impl AsRef<str>) -> Result<FileLock, SharedMemoryBufferNewError> {
        Self::get_lock(Self::reader_lock_path(&name))
    }

    fn get_writer_lock(name: impl AsRef<str>) -> Result<FileLock, SharedMemoryBufferNewError> {
        Self::get_lock(Self::writer_lock_path(&name))
    }

    fn get_lock(lock_path: impl AsRef<Path>) -> Result<FileLock, SharedMemoryBufferNewError> {
        FileLock::open(&lock_path)
            .or_else(|_| FileLock::create(&lock_path))
            .or(Err(SharedMemoryBufferNewError::UnableToLockSharedMemory {
                read_lock_path: lock_path.as_ref().into(),
            }))
    }

    fn try_lock(lock_file: &mut FileLock) -> Result<(), SharedMemoryBufferNewError> {
        lock_file
            .lock()
            .map_err(|_| SharedMemoryBufferNewError::UnableToLockSharedMemory {
                read_lock_path: lock_file.path().into(),
            })
    }

    fn try_unlock(lock_file: &mut FileLock) -> Result<(), SharedMemoryBufferNewError> {
        lock_file
            .unlock()
            .map_err(|_| SharedMemoryBufferNewError::UnableToLockSharedMemory {
                read_lock_path: lock_file.path().into(),
            })
    }

    fn reader_lock_path(name: impl AsRef<str>) -> PathBuf {
        format!("/tmp/{}_reader.lock", name.as_ref()).into()
    }

    fn writer_lock_path(name: impl AsRef<str>) -> PathBuf {
        format!("/tmp/{}_writer.lock", name.as_ref()).into()
    }
}
