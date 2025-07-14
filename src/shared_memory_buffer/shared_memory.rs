use libc::off_t;
use nix::sys::stat::{Mode, umask};
use std::ffi::{CString, c_uint};
use std::mem::forget;
use std::ops::Not;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{debug, instrument, warn};

#[derive(Error, Debug)]
pub enum SharedMemoryCreateError {
    #[error("IO error trying to create shared memory: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid shared memory path: {path}")]
    InvalidSegmentName { path: PathBuf },

    #[error("Shared memory \"{path}\" already exists")]
    AlreadyExists { path: PathBuf },
}

#[derive(Error, Debug)]
pub enum SharedMemoryOpenError {
    #[error("IO error trying to open shared memory: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid shared memory path: {path}")]
    InvalidSegmentName { path: PathBuf },
}

#[derive(Error, Debug)]
pub enum SharedMemoryCloseError {
    #[error("IO error trying to close shared memory: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Error, Debug)]
pub enum SharedMemoryDeleteError {
    #[error("IO error trying to delete shared memory: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid shared memory path: {path}")]
    InvalidSegmentName { path: PathBuf },
}

#[derive(Error, Debug)]
pub enum SharedMemoryCloseAndDeleteError {
    #[error("Error trying to close shared memory: {0}")]
    CloseError(#[from] SharedMemoryCloseError),

    #[error("Error trying to delete shared memory: {0}")]
    DeleteError(#[from] SharedMemoryDeleteError),
}

#[derive(Error, Debug)]
pub enum SharedMemoryMapError {
    #[error("IO error trying to map shared memory: {0}")]
    Io(#[from] std::io::Error),
}

pub struct SharedMemory {
    path: PathBuf,
    size: usize,
    file_descriptor: i32,
}

pub struct MappedSharedMemory {
    shared: SharedMemory,
    mapped_address: *mut libc::c_void,
}

impl SharedMemory {
    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    pub fn size(&self) -> usize {
        self.size
    }

    /// Anything that would fail like an invalid name will just output false
    #[instrument(skip_all, fields(path = ?path.as_ref().display()))]
    pub fn exists(path: impl AsRef<Path>) -> bool {
        let path = path.as_ref();

        debug!("Checking if shared memory exists");

        debug!("Turning Rust Path into C string");
        let c_name = match Self::path_to_cstring(&path) {
            Some(c_name) => c_name,
            None => {
                warn!("Failed to convert Rust Path to C string");
                return false;
            }
        };

        debug!("Attempting to open shared memory in read-only mode");
        let file_descriptor = unsafe { libc::shm_open(c_name.as_ptr(), libc::O_RDONLY, 0) };

        if file_descriptor < 0 {
            debug!("Failed to open shared memory. Assuming it does not exist");
            false
        } else {
            debug!("Successfully opened shared memory. It exists");
            debug!("Closing shared memory file descriptor");
            unsafe {
                libc::close(file_descriptor);
            }
            true
        }
    }

    // Create a new shared memory segment
    #[instrument(skip_all, fields(
        path = ?path.as_ref().display(),
        size = size,
        permission_mode = ?permission_mode.bits()
    ))]
    pub fn create(
        path: impl AsRef<Path>,
        size: usize,
        permission_mode: Mode,
    ) -> Result<Self, SharedMemoryCreateError> {
        let path = path.as_ref();

        debug!("Creating shared memory");

        debug!("Turning Rust Path into C string");
        let c_name = Self::path_to_cstring(&path)
            .ok_or_else(|| SharedMemoryCreateError::InvalidSegmentName {
                path: path.to_owned(),
            })
            .map_err(|error| {
                warn!("Failed to convert Rust Path to C string");
                error
            })?;

        debug!(
            "Creating shared memory with shm_open. Flags \
            O_CREAT | O_RDWR and permission mode {permission_mode:o}"
        );
        let file_descriptor = unsafe {
            libc::shm_open(
                c_name.as_ptr(),
                libc::O_CREAT | libc::O_RDWR,
                permission_mode.bits() as c_uint,
            )
        };

        if file_descriptor < 0 {
            warn!("Failed to create shared memory");
            let err = std::io::Error::last_os_error();
            return Err(SharedMemoryCreateError::Io(err));
        }

        debug!("Setting shared memory size to {} with ftruncate", size);
        if unsafe { libc::ftruncate(file_descriptor, size as off_t) } < 0 {
            warn!("Failed to set shared memory size");
            let err = std::io::Error::last_os_error();
            debug!("Closing shared memory file descriptor");
            unsafe { libc::close(file_descriptor) };
            return Err(SharedMemoryCreateError::Io(err));
        }

        debug!("Successfully created shared memory");
        Ok(SharedMemory {
            path: path.to_owned(),
            size,
            file_descriptor,
        })
    }

    // Open an existing shared memory segment
    #[instrument(skip_all, fields(path = ?path.as_ref().display()))]
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SharedMemoryOpenError> {
        let path = path.as_ref();

        debug!("Opening shared memory");

        debug!("Turning Rust Path into C string");
        let c_name = Self::path_to_cstring(&path)
            .ok_or_else(|| SharedMemoryOpenError::InvalidSegmentName {
                path: path.to_owned(),
            })
            .map_err(|error| {
                warn!("Failed to convert Rust Path to C string");
                error
            })?;

        debug!("Opening shared memory with shm_open. Flags O_RDWR");
        let file_descriptor = unsafe { libc::shm_open(c_name.as_ptr(), libc::O_RDWR, 0) };

        if file_descriptor < 0 {
            warn!("Failed to open shared memory");
            let err = std::io::Error::last_os_error();
            return Err(SharedMemoryOpenError::Io(err));
        }

        debug!("Getting shared memory size with fstat");
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstat(file_descriptor, &mut stat) } != 0 {
            warn!("Failed to get shared memory size");
            let err = std::io::Error::last_os_error();
            debug!("Closing shared memory file descriptor");
            unsafe { libc::close(file_descriptor) };
            return Err(SharedMemoryOpenError::Io(err));
        }

        let size = stat.st_size as usize;

        debug!("Successfully opened shared memory with size {size}");
        Ok(SharedMemory {
            path: path.to_owned(),
            size,
            file_descriptor,
        })
    }

    #[instrument(skip_all, fields(path = ?path.as_ref().display()))]
    pub fn delete(path: impl AsRef<Path>) -> Result<(), SharedMemoryDeleteError> {
        debug!("Deleting shared memory");

        let path = path.as_ref();

        debug!("Turning Rust Path into C string");
        let c_name = Self::path_to_cstring(&path)
            .ok_or_else(|| SharedMemoryDeleteError::InvalidSegmentName {
                path: path.to_owned(),
            })
            .map_err(|error| {
                warn!("Failed to convert Rust Path to C string");
                error
            })?;

        debug!("Deleting shared memory with shm_unlink");
        let result = unsafe { libc::shm_unlink(c_name.as_ptr()) };

        if result < 0 {
            warn!("Failed to delete shared memory");
            let err = std::io::Error::last_os_error();
            return Err(SharedMemoryDeleteError::Io(err));
        }

        debug!("Successfully deleted shared memory");
        Ok(())
    }

    // Map the shared memory to process address space
    #[instrument(skip_all, fields(path = ?self.path().display(), size = self.size()))]
    pub fn map(self) -> Result<MappedSharedMemory, SharedMemoryMapError> {
        debug!("Mapping shared memory");

        debug!("Mapping shared memory with mmap");
        let mapped_address = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                self.size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                self.file_descriptor,
                0,
            )
        };

        if mapped_address == libc::MAP_FAILED {
            warn!("Failed to map shared memory");
            let err = std::io::Error::last_os_error();
            return Err(SharedMemoryMapError::Io(err));
        }

        debug!("Successfully mapped shared memory");
        Ok(MappedSharedMemory {
            shared: self,
            mapped_address,
        })
    }

    fn path_to_cstring(path: &Path) -> Option<CString> {
        path.to_str().and_then(|s| CString::new(s).ok())
    }
}

impl MappedSharedMemory {
    pub fn path(&self) -> &Path {
        self.shared.path()
    }

    pub fn size(&self) -> usize {
        self.shared.size()
    }

    // Get a slice to the mapped memory
    pub unsafe fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.mapped_address as *const u8, self.size()) }
    }

    // Get a mutable slice to the mapped memory
    pub unsafe fn as_slice_mut(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.mapped_address as *mut u8, self.size()) }
    }
}

impl Drop for SharedMemory {
    #[instrument(skip_all, fields(path = ?self.path().display(), size = self.size()))]
    fn drop(&mut self) {
        debug!("Dropping shared memory");

        debug!("Closing shared memory file descriptor");
        let result = unsafe { libc::close(self.file_descriptor) };

        if result != 0 {
            let err = std::io::Error::last_os_error();
            warn!("Failed to close shared memory file descriptor: {err}");
        } else {
            debug!("Successfully closed shared memory file descriptor");
        }
    }
}

impl Drop for MappedSharedMemory {
    #[instrument(skip_all, fields(path = ?self.path().display(), size = self.size()))]
    fn drop(&mut self) {
        debug!("Dropping mapped shared memory");

        debug!("Unmapping shared memory");
        let result = unsafe { libc::munmap(self.mapped_address, self.shared.size) };

        if result == -1 {
            let err = std::io::Error::last_os_error();
            warn!("Failed to unmap shared memory: {err}");
        } else {
            debug!("Successfully unmapped shared memory");
        }

        // The SharedMemory will be dropped automatically after this
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nix::sys::stat::Mode;
    use std::hash::{DefaultHasher, Hash, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};

    // Helper function to create a unique path for shared memory
    fn get_unique_path<T: AsRef<str>>(test_name: T) -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut hasher = DefaultHasher::new();
        test_name.as_ref().hash(&mut hasher);
        timestamp.hash(&mut hasher);
        hasher.finish().to_string()
    }

    #[test]
    fn test_create_success() {
        // Create a shared memory segment with standard parameters
        let path = get_unique_path("test_create_success");
        let size = 4096;
        let mode = Mode::S_IRUSR | Mode::S_IWUSR; // 0o600

        let result = SharedMemory::create(&path, size, mode);
        assert!(
            result.is_ok(),
            "Failed to create shared memory: {:?}",
            result.err()
        );

        let shm = result.unwrap();

        // Verify path
        assert_eq!(shm.path().to_str().unwrap(), path);

        // Verify size
        assert_eq!(shm.size(), size);

        // Try opening it again to verify it exists
        let open_result = SharedMemory::open(&path);
        assert!(
            open_result.is_ok(),
            "Failed to open just-created memory: {:?}",
            open_result.err()
        );
    }

    #[test]
    fn test_create_invalid_path() {
        // Test with an invalid path containing a null byte
        let invalid_path = "\0invalid";
        let size = 4096;
        let mode = Mode::S_IRUSR | Mode::S_IWUSR;

        let result = SharedMemory::create(invalid_path, size, mode);

        // Should fail with InvalidSegmentName
        assert!(result.is_err());
        if let Err(err) = result {
            match err {
                SharedMemoryCreateError::InvalidSegmentName { path: _ } => {
                    // Expected error
                }
                _ => panic!("Expected InvalidSegmentName error, got: {:?}", err),
            }
        }
    }

    #[test]
    fn test_open_success() {
        // First create a shared memory segment
        let path = get_unique_path("test_open_success");
        let requested_size = 4096;
        let mode = Mode::S_IRUSR | Mode::S_IWUSR;

        let shm_create = SharedMemory::create(&path, requested_size, mode)
            .expect("Failed to create shared memory for open test");

        // Then try to open it
        let open_result = SharedMemory::open(&path);
        assert!(
            open_result.is_ok(),
            "Failed to open existing shared memory: {:?}",
            open_result.err()
        );

        let shm_open = open_result.unwrap();

        // Verify path
        assert_eq!(shm_open.path().to_str().unwrap(), path);

        // Verify size - only check it's at least the requested size
        assert!(
            shm_open.size() >= requested_size,
            "Opened shared memory size ({}) should be at least the requested size ({})",
            shm_open.size(),
            requested_size
        );

        // Verify file descriptor is valid
        assert!(shm_open.file_descriptor >= 0);

        // Verify file descriptors are different (not the same handle)
        assert_ne!(
            shm_create.file_descriptor, shm_open.file_descriptor,
            "Open should create a new file descriptor"
        );
    }

    #[test]
    fn test_open_nonexistent() {
        // Try to open a shared memory segment that doesn't exist
        // Generate a path that is very unlikely to exist
        let nonexistent_path = format!("/ne_{}", get_unique_path("test_open_nonexistent"));

        let result = SharedMemory::open(&nonexistent_path);

        // Should fail with an IO error
        assert!(
            result.is_err(),
            "Opening non-existent shared memory should fail"
        );

        if let Err(err) = result {
            match err {
                SharedMemoryOpenError::Io(_) => {
                    // Expected error - this is the correct error type
                    // Specific error message could vary by platform,
                    // so we don't check the exact message
                }
                _ => panic!("Expected IO error, got: {:?}", err),
            }
        }
    }

    #[test]
    fn test_open_invalid_path() {
        // Test with invalid path
        let invalid_path1 = "\0invalid";
        let result1 = SharedMemory::open(invalid_path1);
        assert!(result1.is_err(), "Opening path with null byte should fail");
        if let Err(err) = result1 {
            match err {
                SharedMemoryOpenError::InvalidSegmentName { path: _ } => {
                    // Expected error
                }
                _ => panic!(
                    "Expected InvalidSegmentName error for null byte, got: {:?}",
                    err
                ),
            }
        }
    }

    #[test]
    fn test_delete_nonexistent() {
        // Try to delete a shared memory segment that doesn't exist
        let nonexistent_path = format!("/ne_{}", get_unique_path("test_delete_nonexistent"));

        let result = SharedMemory::delete(&nonexistent_path);

        // Should fail with an IO error
        assert!(
            result.is_err(),
            "Deleting non-existent shared memory should fail"
        );

        if let Err(err) = result {
            match err {
                SharedMemoryDeleteError::Io(_) => {
                    // Expected error - this is the correct error type
                    // Specific error message could vary by platform
                }
                _ => panic!("Expected IO error, got: {:?}", err),
            }
        }
    }

    #[test]
    fn test_delete_invalid_path() {
        // Test with an invalid path containing a null byte
        let invalid_path = "\0invalid";

        let result = SharedMemory::delete(invalid_path);

        // Should fail with InvalidSegmentName
        assert!(result.is_err(), "Deleting with invalid path should fail");

        if let Err(err) = result {
            match err {
                SharedMemoryDeleteError::InvalidSegmentName { path: _ } => {
                    // Expected error
                }
                _ => panic!("Expected InvalidSegmentName error, got: {:?}", err),
            }
        }
    }

    #[test]
    fn test_delete_with_open_handle() {
        // Create a shared memory segment
        let path = get_unique_path("test_delete_with_open_handle");
        let size = 4096;
        let mode = Mode::S_IRUSR | Mode::S_IWUSR;

        // Create first handle
        let shm1 = SharedMemory::create(&path, size, mode)
            .expect("Failed to create shared memory for delete test");

        // Open a second handle to the same segment
        let shm2 =
            SharedMemory::open(&path).expect("Failed to open second handle to shared memory");

        // Delete the shared memory segment while handles are still open
        let delete_result = SharedMemory::delete(&path);
        assert!(
            delete_result.is_ok(),
            "Failed to delete shared memory with open handles: {:?}",
            delete_result.err()
        );

        // Verify the segment is deleted by name (can't open it again)
        let open_result = SharedMemory::open(&path);
        assert!(
            open_result.is_err(),
            "Shared memory still exists by name after delete"
        );

        // But existing handles should still be valid and usable
        // Check if file descriptors are still valid
        assert!(
            shm1.file_descriptor >= 0,
            "First file descriptor should still be valid"
        );
        assert!(
            shm2.file_descriptor >= 0,
            "Second file descriptor should still be valid"
        );

        // Try to write to the segment using one handle and read from the other
        // to verify they're still usable and connected to the same memory

        // First, map both handles
        let mapped1_result = shm1.map();
        assert!(
            mapped1_result.is_ok(),
            "Failed to map first handle after delete"
        );

        let mapped2_result = shm2.map();
        assert!(
            mapped2_result.is_ok(),
            "Failed to map second handle after delete"
        );

        // Write data using first mapping
        if let (Ok(mut mapped1), Ok(mapped2)) = (mapped1_result, mapped2_result) {
            unsafe {
                // Write a test pattern to the first mapping
                let slice1 = mapped1.as_slice_mut();
                if !slice1.is_empty() {
                    slice1[0] = 0xAA;
                    slice1[1] = 0xBB;
                    slice1[2] = 0xCC;
                    slice1[3] = 0xDD;

                    // Read it back from the second mapping to verify they share memory
                    let slice2 = mapped2.as_slice();
                    if !slice2.is_empty() {
                        println!(
                            "Values after delete: {:02X} {:02X} {:02X} {:02X}",
                            slice2[0], slice2[1], slice2[2], slice2[3]
                        );

                        // Verify the values match
                        assert_eq!(
                            slice2[0], 0xAA,
                            "Memory not shared between handles after delete"
                        );
                        assert_eq!(
                            slice2[1], 0xBB,
                            "Memory not shared between handles after delete"
                        );
                        assert_eq!(
                            slice2[2], 0xCC,
                            "Memory not shared between handles after delete"
                        );
                        assert_eq!(
                            slice2[3], 0xDD,
                            "Memory not shared between handles after delete"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_double_delete() {
        // Create a shared memory segment
        let path = get_unique_path("test_double_delete");
        let size = 4096;
        let mode = Mode::S_IRUSR | Mode::S_IWUSR;

        // Create the shared memory segment
        {
            let create_result = SharedMemory::create(&path, size, mode);
            assert!(
                create_result.is_ok(),
                "Failed to create shared memory for double delete test"
            );

            // Close it to ensure we can delete it cleanly
        }

        // First delete - should succeed
        let delete_result1 = SharedMemory::delete(&path);
        assert!(
            delete_result1.is_ok(),
            "First delete operation failed: {:?}",
            delete_result1.err()
        );

        // Second delete - should fail with an appropriate error
        let delete_result2 = SharedMemory::delete(&path);
        assert!(
            delete_result2.is_err(),
            "Second delete unexpectedly succeeded when it should fail"
        );

        // Verify the error type is appropriate (should be an IO error)
        if let Err(err) = delete_result2 {
            match err {
                SharedMemoryDeleteError::Io(io_err) => {
                    // This is the expected error type
                    println!("Second delete failed with expected IO error: {}", io_err);

                    // On most systems, this should be "No such file or directory" (ENOENT)
                    // But we don't assert on the specific error message as it might vary by platform
                }
                _ => {
                    // Other error types are acceptable as long as it fails
                    println!("Second delete failed with error: {:?}", err);
                }
            }
        }

        // Verify we can't open it anymore
        let open_result = SharedMemory::open(&path);
        assert!(
            open_result.is_err(),
            "Shared memory still exists after deleting twice"
        );
    }

    #[test]
    fn test_delete_same_path_recreate() {
        // Test delete and then recreate with the same path
        let path = get_unique_path("test_delete_same_path_recreate");
        let size1 = 4096;
        let size2 = 8192; // Different size for verification
        let mode = Mode::S_IRUSR | Mode::S_IWUSR;

        // Create first shared memory segment
        {
            let shm1 = SharedMemory::create(&path, size1, mode)
                .expect("Failed to create first shared memory");

            // Close it
        }

        // Delete it
        let delete_result = SharedMemory::delete(&path);
        assert!(
            delete_result.is_ok(),
            "Failed to delete shared memory: {:?}",
            delete_result.err()
        );

        // Create a new segment with the same path but different size
        let create_result2 = SharedMemory::create(&path, size2, mode);
        assert!(
            create_result2.is_ok(),
            "Failed to recreate shared memory after delete: {:?}",
            create_result2.err()
        );

        if let Ok(shm2) = create_result2 {
            // Verify it has the new size, proving it's a fresh segment
            assert!(
                shm2.size() >= size2,
                "Recreated segment size ({}) should be at least the new requested size ({})",
                shm2.size(),
                size2
            );
        }
    }
}
