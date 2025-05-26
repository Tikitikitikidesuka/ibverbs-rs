use crate::shared_memory_buffer::buffer_status::{CircularBufferStatus, PtrStatus};
use crate::shared_memory_buffer::file_lock::FileLock;
use crate::shared_memory_buffer::shared_memory::{MappedSharedMemory, SharedMemory};
use crate::utils;
use log::{debug, error, info, trace};
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
    ) -> Result<SharedMemoryReadBuffer, SharedMemoryBufferNewError> {
        let name = name.into();
        info!("Creating new read buffer for '{}'", name);

        // Lock the reader
        debug!("Acquiring reader lock for buffer '{}'", name);
        let mut file_lock = Self::get_reader_lock(&name)?;
        trace!("Got reader lock file for buffer '{}'", name);

        Self::try_lock(&mut file_lock)?;
        debug!("Successfully locked reader for buffer '{}'", name);

        // Open and map shared memory
        debug!("Opening and mapping shared memory for buffer '{}'", name);
        let mut shared_memory = Self::open_mapped_shared_memory(&name)?;
        debug!("Successfully mapped shared memory for buffer '{}', size: {} bytes",
               name, shared_memory.size());

        // Check it has enough data for the status
        let required_size = size_of::<CircularBufferStatus>();
        trace!("Validating shared memory size: {} bytes, required: {} bytes",
               shared_memory.size(), required_size);

        if shared_memory.size() < required_size {
            error!("Shared memory for buffer '{}' too small: {} bytes < {} bytes required",
                   name, shared_memory.size(), required_size);
            return Err(SharedMemoryBufferNewError::MemoryNotBigEnough {
                minimum_size: required_size,
                size: shared_memory.size(),
            });
        }

        // Get buffer usable size from the header
        trace!("Reading buffer metadata from shared memory header for '{}'", name);
        let size = unsafe {
            let status_ptr = shared_memory.as_slice().as_ptr() as *const CircularBufferStatus;
            let size = (&*status_ptr).size();
            trace!("Buffer '{}' usable size from header: {} bytes", name, size);
            size
        };

        // Get buffer alignment 2pow from the header
        let alignment_2pow = unsafe {
            let status_ptr = shared_memory.as_slice().as_ptr() as *const CircularBufferStatus;
            let alignment = (&*status_ptr).alignment_2pow();
            trace!("Buffer '{}' alignment from header: 2^{} bytes", name, alignment);
            alignment
        };

        // Get buffer offset taking header and alignment into account
        let start_address = unsafe { shared_memory.as_slice().as_ptr() as usize };
        trace!("Calculating buffer pointer for '{}': start_address={:#x}, header_size={}, alignment=2^{}",
               name, start_address, size_of::<CircularBufferStatus>(), alignment_2pow);

        let buffer_ptr = utils::align_up_2pow(
            start_address + size_of::<CircularBufferStatus>(),
            alignment_2pow as u8,
        );

        debug!("Buffer '{}' data region at address {:#x}, size {} bytes",
               name, buffer_ptr, size);

        // Create unsized buffer
        info!("Successfully created read buffer for '{}' (size: {} bytes, alignment: 2^{})",
              name, size, alignment_2pow);

        Ok(SharedMemoryReadBuffer {
            shmem_buffer: SharedMemoryBuffer {
                name: name.into(),
                shared_memory,
                file_lock,
                size,
                alignment_2pow: alignment_2pow as u8,
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
        info!("Creating new write buffer for '{}' (size: {} bytes, alignment: 2^{})",
              name, size, alignment_2pow);

        // Lock the writer
        debug!("Acquiring writer and reader locks for buffer '{}'", name);
        let mut writer_lock = Self::get_writer_lock(&name)?;
        let mut reader_lock = Self::get_reader_lock(&name)?;

        trace!("Locking writer for buffer '{}'", name);
        Self::try_lock(&mut writer_lock)?;
        debug!("Successfully locked writer for buffer '{}'", name);

        trace!("Locking reader for buffer '{}'", name);
        Self::try_lock(&mut reader_lock)?;
        debug!("Successfully locked reader for buffer '{}'", name);

        // Delete any existing shared memory (in case of unclean shutdown)
        trace!("Checking if shared memory exists for buffer '{}'", name);
        if SharedMemory::exists(&name) {
            debug!("Existing shared memory found for buffer '{}', deleting", name);
            SharedMemory::delete(&name).map_err(|e| {
                error!("Failed to delete existing shared memory for buffer '{}': {:?}", name, e);
                SharedMemoryBufferNewError::UnableToAcquireSharedMemory {
                    shared_memory_path: name.clone().into(),
                }
            })?;
            debug!("Successfully deleted existing shared memory for buffer '{}'", name);
        } else {
            trace!("No existing shared memory found for buffer '{}'", name);
        }

        // Create and map shared memory
        debug!("Creating and mapping new shared memory for buffer '{}'", name);
        let mut shared_memory = Self::create_mapped_shared_memory(&name, size, alignment_2pow)?;
        debug!("Successfully created and mapped shared memory for buffer '{}', total size: {} bytes",
               name, shared_memory.size());

        // Initialize the shared memory
        debug!("Initializing shared memory header for buffer '{}'", name);
        Self::initialize_shared_memory(&mut shared_memory, alignment_2pow, size)?;
        debug!("Successfully initialized shared memory for buffer '{}'", name);

        // Release reader lock when initialized
        trace!("Releasing reader lock for buffer '{}'", name);
        Self::try_unlock(&mut reader_lock)?;
        debug!("Successfully released reader lock for buffer '{}'", name);

        // Get buffer offset taking header and alignment into account
        let start_address = unsafe { shared_memory.as_slice().as_ptr() as usize };
        trace!("Calculating buffer pointer for '{}': start_address={:#x}, header_size={}, alignment=2^{}",
               name, start_address, size_of::<CircularBufferStatus>(), alignment_2pow);

        let buffer_ptr = utils::align_up_2pow(
            start_address + size_of::<CircularBufferStatus>(),
            alignment_2pow as u8,
        );

        debug!("Buffer '{}' data region at address {:#x}, size {} bytes",
               name, buffer_ptr, size);

        info!("Successfully created write buffer for '{}' (size: {} bytes, alignment: 2^{})",
              name, size, alignment_2pow);

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
        trace!("Drop called on SharedMemoryWriteBuffer for '{}'", self.shmem_buffer.name);

        // Unlink the shared memory - this removes it from the filesystem
        // Existing mappings remain valid until unmapped
        debug!("Deleting shared memory for buffer '{}'", self.shmem_buffer.name);
        match SharedMemory::delete(&self.shmem_buffer.name) {
            Ok(()) => {
                debug!("Successfully deleted shared memory for buffer '{}'", self.shmem_buffer.name);
            }
            Err(e) => {
                error!("Failed to delete shared memory for buffer '{}': {:?}",
                       self.shmem_buffer.name, e);
            }
        }

        // The writer lock will be released automatically when file_lock drops
        // The shared memory mapping will be unmapped when shared_memory drops
        debug!("Completed cleanup for write buffer '{}'", self.shmem_buffer.name);
    }
}

impl SharedMemoryBuffer {
    pub unsafe fn as_slice(&self) -> &[u8] {
        trace!("Getting immutable slice to buffer '{}' data region (size: {} bytes)",
               self.name, self.size);
        unsafe { std::slice::from_raw_parts(self.buffer_ptr as *const u8, self.size) }
    }

    pub unsafe fn as_slice_mut(&mut self) -> &mut [u8] {
        trace!("Getting mutable slice to buffer '{}' data region (size: {} bytes)",
               self.name, self.size);
        unsafe { std::slice::from_raw_parts_mut(self.buffer_ptr as *mut u8, self.size) }
    }

    pub fn read_status(&self) -> PtrStatus {
        trace!("Reading read status for buffer '{}'", self.name);
        let status = self.status_ref().read_status();
        trace!("Buffer '{}' read status: ptr={}, wrap={}",
               self.name, status.ptr(), status.wrap());
        status
    }

    pub fn write_status(&self) -> PtrStatus {
        trace!("Reading write status for buffer '{}'", self.name);
        let status = self.status_ref().write_status();
        trace!("Buffer '{}' write status: ptr={}, wrap={}",
               self.name, status.ptr(), status.wrap());
        status
    }

    pub fn set_read_status(&mut self, status: PtrStatus) {
        debug!("Setting read status for buffer '{}': ptr={}, wrap={}",
               self.name, status.ptr(), status.wrap());
        self.status_ref_mut().set_read_status(status);
        trace!("Successfully updated read status for buffer '{}'", self.name);
    }

    pub fn set_write_status(&mut self, status: PtrStatus) {
        debug!("Setting write status for buffer '{}': ptr={}, wrap={}",
               self.name, status.ptr(), status.wrap());
        self.status_ref_mut().set_write_status(status);
        trace!("Successfully updated write status for buffer '{}'", self.name);
    }

    fn status_ref(&self) -> &CircularBufferStatus {
        trace!("Getting immutable reference to status header for buffer '{}'", self.name);
        unsafe { &*(self.shared_memory.as_slice().as_ptr() as *const CircularBufferStatus) }
    }

    fn status_ref_mut(&mut self) -> &mut CircularBufferStatus {
        trace!("Getting mutable reference to status header for buffer '{}'", self.name);
        unsafe { &mut *(self.shared_memory.as_slice().as_ptr() as *mut _) }
    }
}

impl SharedMemoryReadBuffer {
    pub unsafe fn as_slice(&self) -> &[u8] {
        self.shmem_buffer.as_slice()
    }

    pub fn size(&self) -> usize {
        self.shmem_buffer.size
    }

    pub fn alignment_2pow(&self) -> u8 {
        self.shmem_buffer.alignment_2pow
    }

    pub fn read_status(&self) -> PtrStatus {
        self.shmem_buffer.read_status()
    }

    pub fn write_status(&self) -> PtrStatus {
        self.shmem_buffer.write_status()
    }

    pub fn set_read_status(&mut self, status: PtrStatus) {
        self.shmem_buffer.set_read_status(status);
    }

    pub fn tail_free_space(&self) -> usize {
        trace!("Calculating tail free space for read buffer '{}'", self.shmem_buffer.name);
        let space = self.shmem_buffer.status_ref().tail_free_space();
        trace!("Buffer '{}' tail free space: {} bytes", self.shmem_buffer.name, space);
        space
    }

    pub fn head_free_space(&self) -> usize {
        trace!("Calculating head free space for read buffer '{}'", self.shmem_buffer.name);
        let space = self.shmem_buffer.status_ref().head_free_space();
        trace!("Buffer '{}' head free space: {} bytes", self.shmem_buffer.name, space);
        space
    }

    pub fn available_to_read(&self) -> usize {
        trace!("Calculating available data to read for buffer '{}'", self.shmem_buffer.name);
        let available = self.shmem_buffer.status_ref().available_to_read();
        trace!("Buffer '{}' available to read: {} bytes", self.shmem_buffer.name, available);
        available
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

    pub fn alignment_2pow(&self) -> u8 {
        self.shmem_buffer.alignment_2pow
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
        trace!("Calculating tail free space for write buffer '{}'", self.shmem_buffer.name);
        let space = self.shmem_buffer.status_ref().tail_free_space();
        trace!("Buffer '{}' tail free space: {} bytes", self.shmem_buffer.name, space);
        space
    }

    pub fn head_free_space(&self) -> usize {
        trace!("Calculating head free space for write buffer '{}'", self.shmem_buffer.name);
        let space = self.shmem_buffer.status_ref().head_free_space();
        trace!("Buffer '{}' head free space: {} bytes", self.shmem_buffer.name, space);
        space
    }

    pub fn available_to_write(&self) -> usize {
        trace!("Calculating available space to write for buffer '{}'", self.shmem_buffer.name);
        let available = self.shmem_buffer.status_ref().available_to_write();
        trace!("Buffer '{}' available to write: {} bytes", self.shmem_buffer.name, available);
        available
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
        debug!("Initializing shared memory header (alignment: 2^{}, buffer size: {} bytes)",
               alignment_2pow, size);

        // Use the same alignment calculations as create_mapped_shared_memory
        let aligned_buffer_size = utils::align_up_2pow(size, alignment_2pow);
        trace!("Aligned buffer size: {} bytes (requested: {}, alignment: 2^{})",
               aligned_buffer_size, size, alignment_2pow);

        // Create the initial status with the actual buffer size
        let initial_status = CircularBufferStatus::new(
            aligned_buffer_size,
            alignment_2pow as usize,
            0, // id - you might want to make this configurable
        );

        trace!("Created initial status: size={}, alignment={}, id={}",
               aligned_buffer_size, alignment_2pow, 0);

        // Write the status header at the beginning of shared memory
        unsafe {
            let status_ptr = shared_memory.as_slice_mut().as_mut_ptr() as *mut CircularBufferStatus;
            trace!("Writing status header to shared memory at address: {:p}", status_ptr);
            std::ptr::write(status_ptr, initial_status);
            trace!("Successfully wrote status header to shared memory");
        }

        debug!("Successfully initialized shared memory header");
        Ok(())
    }

    fn open_mapped_shared_memory(
        name: impl Into<String>,
    ) -> Result<MappedSharedMemory, SharedMemoryBufferNewError> {
        let name = name.into();
        debug!("Opening and mapping existing shared memory for '{}'", name);

        trace!("Opening shared memory segment '{}'", name);
        let shared_memory = SharedMemory::open(&name).map_err(|e| {
            error!("Failed to open shared memory '{}': {:?}", name, e);
            SharedMemoryBufferNewError::UnableToAcquireSharedMemory {
                shared_memory_path: name.clone().into(),
            }
        })?;

        debug!("Successfully opened shared memory '{}', size: {} bytes",
               name, shared_memory.size());

        trace!("Mapping shared memory '{}' to process address space", name);
        let mapped = shared_memory.map().map_err(|e| {
            error!("Failed to map shared memory '{}': {:?}", name, e);
            SharedMemoryBufferNewError::UnableToMapSharedMemory {
                shared_memory_path: name.clone().into(),
            }
        })?;

        debug!("Successfully mapped shared memory '{}' at address: {:p}",
               name, unsafe { mapped.as_slice().as_ptr() });

        Ok(mapped)
    }

    fn create_mapped_shared_memory(
        name: impl Into<String>,
        size: usize,
        alignment_2pow: u8, // Add this parameter
    ) -> Result<MappedSharedMemory, SharedMemoryBufferNewError> {
        let name = name.into();
        debug!("Creating and mapping new shared memory for '{}' (requested size: {} bytes, alignment: 2^{})",
               name, size, alignment_2pow);

        // Calculate total size needed (header + padding + buffer + padding)
        let header_size = size_of::<CircularBufferStatus>();
        let aligned_header_size = utils::align_up_2pow(header_size, alignment_2pow);
        let aligned_buffer_size = utils::align_up_2pow(size, alignment_2pow);
        let total_size = aligned_header_size + aligned_buffer_size;

        debug!("Size calculations for '{}': header={} bytes, aligned_header={} bytes, \
                aligned_buffer={} bytes, total={} bytes",
               name, header_size, aligned_header_size, aligned_buffer_size, total_size);

        trace!("Creating shared memory segment '{}' with size {} bytes", name, total_size);
        let shared_memory = SharedMemory::create(&name, total_size, PERMISSION_MODE).map_err(|e| {
            error!("Failed to create shared memory '{}': {:?}", name, e);
            SharedMemoryBufferNewError::UnableToAcquireSharedMemory {
                shared_memory_path: name.clone().into(),
            }
        })?;

        debug!("Successfully created shared memory '{}', actual size: {} bytes",
               name, shared_memory.size());

        trace!("Mapping shared memory '{}' to process address space", name);
        let mapped = shared_memory.map().map_err(|e| {
            error!("Failed to map shared memory '{}': {:?}", name, e);
            SharedMemoryBufferNewError::UnableToMapSharedMemory {
                shared_memory_path: name.clone().into(),
            }
        })?;

        debug!("Successfully mapped shared memory '{}' at address: {:p}",
               name, unsafe { mapped.as_slice().as_ptr() });

        Ok(mapped)
    }

    fn get_reader_lock(name: impl AsRef<str>) -> Result<FileLock, SharedMemoryBufferNewError> {
        let name = name.as_ref();
        debug!("Getting reader lock for buffer '{}'", name);
        let lock_path = Self::reader_lock_path(name);
        debug!("Reader lock path for '{}': {:?}", name, lock_path);
        Self::get_lock(lock_path)
    }

    fn get_writer_lock(name: impl AsRef<str>) -> Result<FileLock, SharedMemoryBufferNewError> {
        let name = name.as_ref();
        debug!("Getting writer lock for buffer '{}'", name);
        let lock_path = Self::writer_lock_path(name);
        debug!("Writer lock path for '{}': {:?}", name, lock_path);
        Self::get_lock(lock_path)
    }

    fn get_lock(lock_path: impl AsRef<Path>) -> Result<FileLock, SharedMemoryBufferNewError> {
        let lock_path_ref = lock_path.as_ref();
        trace!("Attempting to get lock file: {:?}", lock_path_ref);

        // Try to open existing lock file first
        trace!("Trying to open existing lock file: {:?}", lock_path_ref);
        match FileLock::open(&lock_path_ref) {
            Ok(lock) => {
                debug!("Successfully opened existing lock file: {:?}", lock_path_ref);
                Ok(lock)
            }
            Err(open_err) => {
                trace!("Failed to open existing lock file: {:?}, trying to create: {:?}",
                       open_err, lock_path_ref);

                match FileLock::create(&lock_path_ref) {
                    Ok(lock) => {
                        debug!("Successfully created new lock file: {:?}", lock_path_ref);
                        Ok(lock)
                    }
                    Err(create_err) => {
                        error!("Failed to open or create lock file {:?}: open={:?}, create={:?}",
                               lock_path_ref, open_err, create_err);
                        Err(SharedMemoryBufferNewError::UnableToLockSharedMemory {
                            read_lock_path: lock_path_ref.into(),
                        })
                    }
                }
            }
        }
    }

    fn try_lock(lock_file: &mut FileLock) -> Result<(), SharedMemoryBufferNewError> {
        let lock_path = lock_file.path().to_owned();
        debug!("Attempting to acquire lock: {:?}", lock_path);

        trace!("Calling lock() on file: {:?}", lock_path);
        match lock_file.lock() {
            Ok(()) => {
                debug!("Successfully acquired lock: {:?}", lock_path);
                Ok(())
            }
            Err(e) => {
                error!("Failed to acquire lock {:?}: {:?}", lock_path, e);
                Err(SharedMemoryBufferNewError::UnableToLockSharedMemory {
                    read_lock_path: lock_path.into(),
                })
            }
        }
    }

    fn try_unlock(lock_file: &mut FileLock) -> Result<(), SharedMemoryBufferNewError> {
        let lock_path = lock_file.path().to_owned();
        debug!("Attempting to release lock: {:?}", lock_path);

        trace!("Calling unlock() on file: {:?}", lock_path);
        match lock_file.unlock() {
            Ok(()) => {
                debug!("Successfully released lock: {:?}", lock_path);
                Ok(())
            }
            Err(e) => {
                error!("Failed to release lock {:?}: {:?}", lock_path, e);
                Err(SharedMemoryBufferNewError::UnableToLockSharedMemory {
                    read_lock_path: lock_path.into(),
                })
            }
        }
    }

    fn reader_lock_path(name: impl AsRef<str>) -> PathBuf {
        let path = format!("/tmp/{}_reader.lock", name.as_ref()).into();
        trace!("Generated reader lock path for '{}': {:?}", name.as_ref(), path);
        path
    }

    fn writer_lock_path(name: impl AsRef<str>) -> PathBuf {
        let path = format!("/tmp/{}_writer.lock", name.as_ref()).into();
        trace!("Generated writer lock path for '{}': {:?}", name.as_ref(), path);
        path
    }
}