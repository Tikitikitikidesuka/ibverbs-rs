use crate::shared_memory_buffer::buffer_status::CircularBufferStatus;
use crate::shared_memory_buffer::file_lock::FileLock;
use crate::shared_memory_buffer::shared_memory::{MappedSharedMemory, SharedMemory};
use nix::sys::stat::Mode;
use std::path::{Path, PathBuf};
use thiserror::Error;

const PERMISSION_MODE: Mode =
    Mode::S_IRUSR | Mode::S_IWUSR | Mode::S_IRGRP | Mode::S_IWGRP | Mode::S_IROTH | Mode::S_IWOTH;

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

pub struct SharedMemoryBuffer;

pub struct SharedMemoryBufferReader {
    name: String,
    shared_memory: MappedSharedMemory,
    file_lock: FileLock,
}

pub struct SharedMemoryBufferWriter {
    name: String,
    shared_memory: MappedSharedMemory,
    file_lock: FileLock,
}

unsafe impl Send for SharedMemoryBufferReader {}
unsafe impl Send for SharedMemoryBufferWriter {}
unsafe impl Sync for SharedMemoryBufferReader {}
unsafe impl Sync for SharedMemoryBufferWriter {}

impl SharedMemoryBuffer {
    pub fn new_reader<T: Into<String>>(
        name: T,
        size: usize,
        alignment_2pow: usize,
    ) -> Result<SharedMemoryBufferReader, SharedMemoryBufferNewError> {
        // Lock the reader
        let mut file_lock = Self::get_reader_lock(&name)?;
        Self::try_lock(&mut file_lock)?;

        // Acquire/create and map shared memory
        let mut shared_memory = Self::setup_mapped_shared_memory(&name, size)?;

        // Check it has enough data for the status
        if shared_memory.size() < size_of::<CircularBufferStatus>() {
            return Err(SharedMemoryBufferNewError::MemoryNotBigEnough {
                minimum_size: size_of::<CircularBufferStatus>(),
                size,
            });
        }

        Ok(SharedMemoryBufferReader {
            name: name.into(),
            shared_memory,
            file_lock,
        })
    }

    pub fn new_writer<T: Into<String>>(
        name: T,
        size: usize,
        alignment_2pow: usize,
    ) -> Result<SharedMemoryBufferWriter, SharedMemoryBufferNewError> {
        // Lock the writer
        let mut writer_lock = Self::get_writer_lock(&name)?;
        let mut reader_lock = Self::get_reader_lock(&name)?;
        Self::try_lock(&mut writer_lock)?;
        Self::try_lock(&mut reader_lock)?;

        // Acquire/create and map shared memory
        let mut shared_memory = Self::setup_mapped_shared_memory(&name, size)?;

        // Initialize the shared memory
        Self::initialize_shared_memory(&mut shared_memory)?;

        // Release reader lock when initialized
        Self::try_unlock(&mut reader_lock)?;

        Ok(SharedMemoryBufferWriter {
            name: name.into(),
            shared_memory,
            file_lock,
        })
    }
}

impl SharedMemoryBufferReader {
    pub fn read_status(&self) -> &CircularBufferStatus {
        unsafe { &*(self.shared_memory.as_slice().as_ptr() as *const CircularBufferStatus) }
    }
}

impl SharedMemoryBufferWriter {}

//impl Drop for SharedMemoryBuffer {
// Files unlock automatically with their drop
// Shared memory unmaps and closes automatically with its drop
//}

impl SharedMemoryBuffer {
    fn initialize_shared_memory(
        shared_memory: &mut MappedSharedMemory,
    ) -> Result<(), SharedMemoryBufferNewError> {
        // TODO: WHEN IMPLEMENTING THE WRITER
        todo!()
    }

    fn setup_mapped_shared_memory(
        name: impl Into<String>,
        size: usize,
    ) -> Result<MappedSharedMemory, SharedMemoryBufferNewError> {
        SharedMemory::open(&name)
            .or_else(SharedMemory::create(&name, size, PERMISSION_MODE))
            .or_else(Err(
                SharedMemoryBufferNewError::UnableToAcquireSharedMemory {
                    shared_memory_path: name.clone().into(),
                },
            ))?
            .map()
            .or_else(Err(SharedMemoryBufferNewError::UnableToMapSharedMemory {
                shared_memory_path: name.clone().into(),
            }))?;
    }

    fn get_reader_lock(name: impl AsRef<str>) -> Result<FileLock, SharedMemoryBufferNewError> {
        Self::get_lock(Self::reader_lock_path(&name))
    }

    fn get_writer_lock(name: impl AsRef<str>) -> Result<FileLock, SharedMemoryBufferNewError> {
        Self::get_lock(Self::writer_lock_path(&name))
    }

    fn get_lock(lock_path: impl AsRef<Path>) -> Result<FileLock, SharedMemoryBufferNewError> {
        FileLock::open(&lock_path)
            .or_else(FileLock::create(&lock_path))
            .or_else(Err(SharedMemoryBufferNewError::UnableToLockSharedMemory {
                read_lock_path: lock_path.clone(),
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
        format!("/tmp/{}_reader.lock", name).into()
    }

    fn writer_lock_path(name: impl AsRef<str>) -> PathBuf {
        format!("/tmp/{}_writer.lock", name).into()
    }
}
