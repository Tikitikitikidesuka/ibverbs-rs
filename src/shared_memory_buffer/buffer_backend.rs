use crate::shared_memory_buffer::buffer_status::{CircularBufferStatus, PtrStatus};
use crate::shared_memory_buffer::file_lock::LockFile;
use crate::shared_memory_buffer::shared_memory::{MappedSharedMemory, SharedMemory};
use crate::utils;
use log::debug;
use nix::sys::stat::Mode;
use std::path::PathBuf;
use thiserror::Error;
use tracing::field::debug;
use tracing::{instrument, warn};

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
    file_lock: LockFile,
    size: usize,
    alignment_pow2: u8,
    buffer_ptr: *const u8,
}

pub struct SharedMemoryReadBuffer {
    shmem_buffer: SharedMemoryBuffer,
}

pub struct SharedMemoryWriteBuffer {
    shmem_buffer: SharedMemoryBuffer,
}

impl SharedMemoryBuffer {
    #[instrument(skip_all, fields(name = ?name.as_ref()))]
    pub fn new_read_buffer<T: AsRef<str>>(
        name: T,
    ) -> Result<SharedMemoryReadBuffer, SharedMemoryBufferNewError> {
        let name = name.as_ref();

        debug!("Creating read buffer for shared memory");

        debug!("Locking reader");
        let file_lock = Self::try_lock_reader(&name).map_err(|error| {
            warn!("Failed to lock the reader");
            error
        })?;

        debug!("Opening and mapping shared memory");
        let shared_memory = Self::open_mapped_shared_memory(name).map_err(|error| {
            warn!("Failed to open and map shared memory");
            error
        })?;

        debug!("Checking shared memory size is enough for the status structure");
        let required_size = size_of::<CircularBufferStatus>();
        if shared_memory.size() < required_size {
            warn!("Shared memory size is not big enough for the status structure");
            return Err(SharedMemoryBufferNewError::MemoryNotBigEnough {
                minimum_size: required_size,
                size: shared_memory.size(),
            });
        }

        debug!("Getting buffer usable size");
        let size = unsafe {
            let status_ptr = shared_memory.as_slice().as_ptr() as *const CircularBufferStatus;
            let size = (&*status_ptr).size();
            size
        };
        debug!("Buffer usable size: {} Bytes", size);

        debug!("Getting buffer alignment");
        let alignment_pow2 = unsafe {
            let status_ptr = shared_memory.as_slice().as_ptr() as *const CircularBufferStatus;
            let alignment = (&*status_ptr).alignment_pow2();
            alignment
        } as u8;
        debug!("Buffer alignment: 2^{} Bytes", alignment_pow2);

        debug!("Getting buffer usable start address");
        let start_address = unsafe { shared_memory.as_slice().as_ptr() };
        let buffer_ptr = utils::align_up_pow2(
            start_address as usize + size_of::<CircularBufferStatus>(),
            alignment_pow2,
        ) as *const u8;
        debug!("Buffer usable start address {:p}", buffer_ptr);

        debug!("Read buffer created successfully");
        Ok(SharedMemoryReadBuffer {
            shmem_buffer: SharedMemoryBuffer {
                name: name.into(),
                shared_memory,
                file_lock,
                size,
                alignment_pow2,
                buffer_ptr,
            },
        })
    }

    #[instrument(skip_all, fields(
        name = ?name.as_ref(),
        size = size,
        alignment_pow2 = alignment_pow2
    ))]
    pub fn new_write_buffer<T: AsRef<str>>(
        name: T,
        size: usize,
        alignment_pow2: u8,
    ) -> Result<SharedMemoryWriteBuffer, SharedMemoryBufferNewError> {
        let name = name.as_ref();

        debug!("Creating write buffer for shared memory");

        debug!("Locking writer");
        let writer_lock = Self::try_lock_writer(name).map_err(|error| {
            warn!("Failed to lock the writer");
            error
        })?;

        debug!("Locking reader (until buffer is initialized)");
        let reader_lock = Self::try_lock_reader(name).map_err(|error| {
            warn!("Failed to lock the reader");
            error
        })?;

        debug!("Checking if shared memory already exists (in case of unclean shutdown)");
        if SharedMemory::exists(name) {
            debug!("Previous shared memory exists, deleting it");
            SharedMemory::delete(name).map_err(|error| {
                warn!("Failed to delete previous shared memory");
                SharedMemoryBufferNewError::UnableToAcquireSharedMemory {
                    shared_memory_path: name.into()
                }
            })?;
        } else {
            debug!("No previous shared memory found");
        }

        debug!("Creating and mapping shared memory");
        let mut shared_memory = Self::create_mapped_shared_memory(&name, size, alignment_pow2)
            .map_err(|error| {
                warn!("Failed to create and map shared memory");
                error
            })?;

        debug!("Initializing shared memory");
        Self::initialize_shared_memory(&mut shared_memory, alignment_pow2, size).map_err(
            |error| {
                warn!("Failed to initialize shared memory");
                error
            },
        )?;

        debug!("Releasing reader lock");
        drop(reader_lock);

        debug!("Getting buffer usable start address");
        let start_address = unsafe { shared_memory.as_slice().as_ptr() };
        let buffer_ptr = utils::align_up_pow2(
            start_address as usize + size_of::<CircularBufferStatus>(),
            alignment_pow2,
        ) as *const u8;
        debug!("Buffer usable start address {:p}", buffer_ptr);

        debug!("Write buffer created successfully");
        Ok(SharedMemoryWriteBuffer {
            shmem_buffer: SharedMemoryBuffer {
                name: name.into(),
                shared_memory,
                file_lock: writer_lock,
                size,
                alignment_pow2,
                buffer_ptr,
            },
        })
    }
}

impl Drop for SharedMemoryWriteBuffer {
    #[instrument(skip_all, fields(name = ?self.name()))]
    fn drop(&mut self) {
        // Unlink the shared memory - this removes it from the filesystem
        // Existing mappings remain valid until unmapped
        // Technically not necessary for POSIX shared memory
        // It is automatically released when not tied to any process

        debug!("Dropping write buffer, unlinking shared memory");
        match SharedMemory::delete(&self.shmem_buffer.name) {
            Ok(()) => {
                debug!("Shared memory unlinked successfully");
            }
            Err(error) => {
                debug!("Failed to unlink shared memory: {}", error);
            }
        }

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
        let status = self.status_ref().read_status();
        status
    }

    pub fn write_status(&self) -> PtrStatus {
        let status = self.status_ref().write_status();
        status
    }

    pub fn set_read_status(&mut self, status: PtrStatus) {
        self.status_ref_mut().set_read_status(status);
    }

    pub fn set_write_status(&mut self, status: PtrStatus) {
        self.status_ref_mut().set_write_status(status);
    }

    pub fn name(&self) -> &str {
        &self.name
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
        unsafe { self.shmem_buffer.as_slice() }
    }

    pub fn size(&self) -> usize {
        self.shmem_buffer.size
    }

    pub fn alignment_pow2(&self) -> u8 {
        self.shmem_buffer.alignment_pow2
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

    pub fn name(&self) -> &str {
        self.shmem_buffer.name()
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
        unsafe { self.shmem_buffer.as_slice() }
    }

    pub unsafe fn as_slice_mut(&mut self) -> &mut [u8] {
        unsafe { self.shmem_buffer.as_slice_mut() }
    }

    pub fn size(&self) -> usize {
        self.shmem_buffer.size
    }

    pub fn alignment_pow2(&self) -> u8 {
        self.shmem_buffer.alignment_pow2
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

    pub fn name(&self) -> &str {
        self.shmem_buffer.name()
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
    #[instrument(skip_all, fields(
        name = ?shared_memory.path(),
        size = size,
        alignment_pow2 = alignment_pow2
    ))]
    fn initialize_shared_memory(
        shared_memory: &mut MappedSharedMemory,
        alignment_pow2: u8,
        size: usize,
    ) -> Result<(), SharedMemoryBufferNewError> {
        debug!("Initializing shared memory status structure");
        let aligned_buffer_size = utils::align_up_pow2(size, alignment_pow2);
        let initial_status = CircularBufferStatus::new(
            aligned_buffer_size,
            alignment_pow2 as usize,
            0, // id - you might want to make this configurable
        );

        // Write the status header at the beginning of shared memory
        debug!("Writing status structure to shared memory");
        unsafe {
            let status_ptr = shared_memory.as_slice_mut().as_mut_ptr() as *mut CircularBufferStatus;
            std::ptr::write(status_ptr, initial_status);
        }

        debug!("Shared memory initialized successfully");
        Ok(())
    }

    #[instrument(skip_all, fields(name = ?name.as_ref()))]
    fn open_mapped_shared_memory(
        name: impl AsRef<str>,
    ) -> Result<MappedSharedMemory, SharedMemoryBufferNewError> {
        let name = name.as_ref();

        debug!("Opening shared memory");
        let shared_memory = SharedMemory::open(name).map_err(|e| {
            warn!("Failed to open shared memory");
            SharedMemoryBufferNewError::UnableToAcquireSharedMemory {
                shared_memory_path: name.into(),
            }
        })?;

        debug!("Mapping shared memory");
        let mapped = shared_memory.map().map_err(|e| {
            warn!("Failed to map shared memory");
            SharedMemoryBufferNewError::UnableToMapSharedMemory {
                shared_memory_path: name.clone().into(),
            }
        })?;

        debug!("Shared memory opened and mapped successfully");
        Ok(mapped)
    }

    #[instrument(skip_all, fields(
        name = ?name.as_ref(),
        size = size,
        alignment_pow2 = alignment_pow2
    ))]
    fn create_mapped_shared_memory(
        name: impl AsRef<str>,
        size: usize,
        alignment_pow2: u8,
    ) -> Result<MappedSharedMemory, SharedMemoryBufferNewError> {
        let name = name.as_ref();

        debug!("Creating shared memory");

        debug!("Calculating total size with header and padding");
        let header_size = size_of::<CircularBufferStatus>();
        let aligned_header_size = utils::align_up_pow2(header_size, alignment_pow2);
        let aligned_buffer_size = utils::align_up_pow2(size, alignment_pow2);
        let total_size = aligned_header_size + aligned_buffer_size;
        debug!("Total size: {} Bytes", total_size);

        debug!("Creating shared memory");
        let shared_memory =
            SharedMemory::create(&name, total_size, PERMISSION_MODE).map_err(|e| {
                warn!("Failed to create shared memory");
                SharedMemoryBufferNewError::UnableToAcquireSharedMemory {
                    shared_memory_path: name.clone().into(),
                }
            })?;

        debug!("Mapping shared memory");
        let mapped = shared_memory.map().map_err(|e| {
            warn!("Failed to map shared memory");
            SharedMemoryBufferNewError::UnableToMapSharedMemory {
                shared_memory_path: name.clone().into(),
            }
        })?;

        debug!("Shared memory created and mapped successfully");
        Ok(mapped)
    }

    #[instrument(skip_all, fields(name = ?name.as_ref()))]
    fn try_lock_reader(name: impl AsRef<str>) -> Result<LockFile, SharedMemoryBufferNewError> {
        let name = name.as_ref();
        debug!("Trying to lock reader");
        let lock_path = Self::reader_lock_path(name);
        LockFile::try_lock(&lock_path).map_err(|_| {
            warn!("Failed to lock reader");
            SharedMemoryBufferNewError::UnableToLockSharedMemory {
                read_lock_path: lock_path,
            }
        })
    }

    #[instrument(skip_all, fields(name = ?name.as_ref()))]
    fn try_lock_writer(name: impl AsRef<str>) -> Result<LockFile, SharedMemoryBufferNewError> {
        let name = name.as_ref();
        debug!("Trying to lock writer");
        let lock_path = Self::writer_lock_path(name);
        LockFile::try_lock(&lock_path).map_err(|_| {
            warn!("Failed to lock writer");
            SharedMemoryBufferNewError::UnableToLockSharedMemory {
                read_lock_path: lock_path,
            }
        })
    }

    fn reader_lock_path(name: impl AsRef<str>) -> PathBuf {
        format!("/tmp/{}_reader.lock", name.as_ref()).into()
    }

    fn writer_lock_path(name: impl AsRef<str>) -> PathBuf {
        format!("/tmp/{}_writer.lock", name.as_ref()).into()
    }
}
