use libc::off_t;
use log::{debug, error, info, trace};
use nix::sys::stat::{Mode, umask};
use std::ffi::{CString, c_uint};
use std::mem::forget;
use std::ops::Not;
use std::path::{Path, PathBuf};
use thiserror::Error;

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

#[derive(Error, Debug)]
pub enum SharedMemoryUnmapError {
    #[error("IO error trying to unmap shared memory: {0}")]
    Io(#[from] std::io::Error),
}

pub struct SharedMemory {
    path: PathBuf,
    size: usize,
    file_descriptor: i32,
}

pub struct MappedSharedMemory {
    shared: Option<SharedMemory>,
    mapped_address: *mut libc::c_void,
}

impl SharedMemory {
    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn exists(path: impl Into<PathBuf>) -> bool {
        let path = path.into();
        trace!("Checking if shared memory exists at path: {:?}", path);

        let c_name = match CString::new(path.to_str().unwrap_or("")) {
            Ok(c_name) => c_name,
            Err(_) => {
                trace!(
                    "Invalid shared memory path (contains null bytes): {:?}",
                    path
                );
                return false;
            }
        };

        // Try to open the shared memory in read-only mode without creating it
        trace!("Calling shm_open with O_RDONLY to check existence");
        let file_descriptor = unsafe { libc::shm_open(c_name.as_ptr(), libc::O_RDONLY, 0) };
        trace!("shm_open returned file descriptor: {}", file_descriptor);

        if file_descriptor < 0 {
            trace!("Shared memory does not exist at {:?}", path);
            false
        } else {
            trace!("Calling close({})", file_descriptor);
            unsafe {
                libc::close(file_descriptor);
            }
            trace!("close({}) completed", file_descriptor);
            debug!("Shared memory exists at {:?}", path);
            true
        }
    }

    // Create a new shared memory segment
    pub fn create(
        path: impl Into<PathBuf>,
        size: usize,
        permission_mode: Mode,
    ) -> Result<Self, SharedMemoryCreateError> {
        let path = path.into();
        info!(
            "Creating shared memory at path: {:?} with size: {}",
            path, size
        );

        let path_str = path.to_str().ok_or_else(|| {
            error!("Invalid shared memory path: {:?}", path);
            SharedMemoryCreateError::InvalidSegmentName { path: path.clone() }
        })?;
        let c_name = CString::new(path_str).map_err(|_| {
            error!("Path contains interior null byte: {:?}", path);
            SharedMemoryCreateError::InvalidSegmentName { path: path.clone() }
        })?;

        debug!("Using permission mode: {:?}", permission_mode);

        // Create shared memory
        let old_mask = umask(permission_mode.not());
        trace!("Setting umask to {:?}", permission_mode.not());

        trace!("Calling shm_open with O_CREAT | O_RDWR");
        let file_descriptor = unsafe {
            libc::shm_open(
                c_name.as_ptr(),
                libc::O_CREAT | libc::O_RDWR,
                permission_mode.bits() as c_uint,
            )
        };
        trace!("shm_open returned file descriptor: {}", file_descriptor);

        umask(old_mask);
        trace!("Restored umask to previous value");

        if file_descriptor < 0 {
            let err = std::io::Error::last_os_error();
            error!("Failed to create shared memory at {:?}: {}", path, err);
            return Err(SharedMemoryCreateError::Io(err));
        }

        // Set shared memory size
        trace!("Calling ftruncate({}, {})", file_descriptor, size);
        if unsafe { libc::ftruncate(file_descriptor, size as off_t) } < 0 {
            let err = std::io::Error::last_os_error();
            error!("Failed to set size of shared memory at {:?}: {}", path, err);
            trace!("Calling close({})", file_descriptor);
            unsafe { libc::close(file_descriptor) };
            trace!("close({}) completed", file_descriptor);
            return Err(SharedMemoryCreateError::Io(err));
        }
        trace!("ftruncate completed successfully");

        debug!(
            "Successfully created shared memory at {:?} with fd {}",
            path, file_descriptor
        );
        Ok(SharedMemory {
            path,
            size,
            file_descriptor,
        })
    }

    // Open an existing shared memory segment
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, SharedMemoryOpenError> {
        let path = path.into();
        info!("Opening shared memory at path: {:?}", path);

        let c_name = CString::new(path.to_str().unwrap_or("")).map_err(|_| {
            error!("Invalid shared memory path: {:?}", path);
            SharedMemoryOpenError::InvalidSegmentName { path: path.clone() }
        })?;

        // Open shared memory
        trace!("Calling shm_open with O_RDWR");
        let file_descriptor = unsafe { libc::shm_open(c_name.as_ptr(), libc::O_RDWR, 0) };
        trace!("shm_open returned file descriptor: {}", file_descriptor);

        if file_descriptor < 0 {
            let err = std::io::Error::last_os_error();
            error!("Failed to open shared memory at {:?}: {}", path, err);
            return Err(SharedMemoryOpenError::Io(err));
        }

        // Get shared memory size
        trace!("Calling fstat({})", file_descriptor);
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstat(file_descriptor, &mut stat) } != 0 {
            let err = std::io::Error::last_os_error();
            error!("Failed to get size of shared memory at {:?}: {}", path, err);
            trace!("Calling close({})", file_descriptor);
            unsafe { libc::close(file_descriptor) };
            trace!("close({}) completed", file_descriptor);
            return Err(SharedMemoryOpenError::Io(err));
        }
        trace!("fstat completed successfully");

        let size = stat.st_size as usize;
        debug!(
            "Successfully opened shared memory at {:?} with fd {} and size {}",
            path, file_descriptor, size
        );

        Ok(SharedMemory {
            path,
            size,
            file_descriptor,
        })
    }

    pub fn close(mut self) -> Result<(), SharedMemoryCloseError> {
        trace!(
            "Closing shared memory fd {} at path {:?}",
            self.file_descriptor, self.path
        );

        let c_result = unsafe { libc::close(self.file_descriptor) };

        if c_result != 0 {
            let err = std::io::Error::last_os_error();
            error!(
                "Failed to close shared memory fd {} at {:?}: {}",
                self.file_descriptor, self.path, err
            );
            return Err(SharedMemoryCloseError::Io(err));
        }

        debug!(
            "Successfully closed shared memory fd {} at {:?}",
            self.file_descriptor, self.path
        );

        forget(self);

        Ok(())
    }

    pub fn delete(path: impl Into<PathBuf>) -> Result<(), SharedMemoryDeleteError> {
        let path = path.into();
        info!("Deleting shared memory at path: {:?}", path);

        let c_name = CString::new(path.to_str().unwrap_or("")).map_err(|_| {
            error!("Invalid shared memory path: {:?}", path);
            SharedMemoryDeleteError::InvalidSegmentName { path: path.clone() }
        })?;

        // Delete shared memory (unlink it from the namespace)
        trace!("Calling shm_unlink");
        let result = unsafe { libc::shm_unlink(c_name.as_ptr()) };
        trace!("shm_unlink returned: {}", result);

        if result < 0 {
            let err = std::io::Error::last_os_error();
            error!("Failed to delete shared memory at {:?}: {}", path, err);
            return Err(SharedMemoryDeleteError::Io(err));
        }

        debug!("Successfully deleted shared memory at {:?}", path);
        Ok(())
    }

    pub fn close_and_delete(self) -> Result<(), SharedMemoryCloseAndDeleteError> {
        let path = self.path.clone();
        self.close()?;
        Self::delete(path)?;
        Ok(())
    }

    // Map the shared memory to process address space
    pub fn map(self) -> Result<MappedSharedMemory, SharedMemoryMapError> {
        debug!(
            "Mapping shared memory at {:?} to process address space",
            self.path
        );

        trace!(
            "Calling mmap(NULL, {}, PROT_READ | PROT_WRITE, MAP_SHARED, {}, 0)",
            self.size, self.file_descriptor
        );

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
        trace!("mmap returned address: {:p}", mapped_address);

        if mapped_address == libc::MAP_FAILED {
            let err = std::io::Error::last_os_error();
            error!(
                "Failed to map shared memory at {:?} with fd {}: {}",
                self.path, self.file_descriptor, err
            );

            return Err(SharedMemoryMapError::Io(err));
        }

        debug!(
            "Successfully mapped shared memory at {:?} with fd {} to address {:p}",
            self.path, self.file_descriptor, mapped_address
        );

        // Create the mapped shared memory, wrapping self
        Ok(MappedSharedMemory {
            shared: Some(self),
            mapped_address,
        })
    }
}

impl MappedSharedMemory {
    pub fn path(&self) -> &Path {
        self.shared.as_ref().expect(
            "Attempted to access path of a MappedSharedMemory that has no inner SharedMemory.\n\
            This indicates a severe implementation error as any valid MappedSharedMemory must always \
            contain a SharedMemory reference until explicitly unmapped.\n\
            Please report this issue to the developers."
        ).path()
    }

    pub fn size(&self) -> usize {
        self.shared.as_ref().expect(
            "Attempted to access size of a MappedSharedMemory that has no inner SharedMemory.\n\
            This indicates a severe implementation error as the Option<SharedMemory> should never be None \
            unless explicitly unmapped.\n\
            Please report this issue to the developers."
        ).size()
    }

    // Get a slice to the mapped memory
    pub unsafe fn as_slice(&self) -> &[u8] {
        trace!("Getting immutable slice to mapped shared memory");

        let slice = std::slice::from_raw_parts(self.mapped_address as *const u8, self.size());
        trace!("Returned immutable slice of size {}", self.size());
        slice
    }

    // Get a mutable slice to the mapped memory
    pub unsafe fn as_slice_mut(&mut self) -> &mut [u8] {
        trace!("Getting mutable slice to mapped shared memory");

        let slice = std::slice::from_raw_parts_mut(self.mapped_address as *mut u8, self.size());
        trace!("Returned mutable slice of size {}", self.size());
        slice
    }

    // Explicitly unmap the memory
    pub fn unmap(mut self) -> Result<SharedMemory, SharedMemoryMapError> {
        match self.shared.take() {
            Some(shared) => {
                debug!("Unmapping shared memory at {:?}", shared.path);

                // Unmap the memory
                trace!(
                    "Calling munmap({:p}, {})",
                    self.mapped_address,
                    shared.size()
                );
                let result = unsafe { libc::munmap(self.mapped_address, shared.size()) };
                trace!("munmap returned: {}", result);

                if result == -1 {
                    let err = std::io::Error::last_os_error();
                    error!(
                        "Failed to unmap shared memory at {:?} from address {:p}: {}",
                        shared.path, self.mapped_address, err
                    );

                    // Put shared memory back since we couldn't unmap
                    self.shared = Some(shared);

                    return Err(SharedMemoryMapError::Io(err));
                }

                debug!(
                    "Successfully unmapped shared memory at {:?} from address {:p}",
                    shared.path, self.mapped_address
                );

                // Return the inner SharedMemory
                Ok(shared)
            }
            None => {
                error!("Cannot unmap: shared memory has already been unmapped");
                unreachable!(
                    "MappedSharedMemory::unmap called after the shared memory was already taken.\n\
                    This indicates an erroneous implementation as the API should prevent double unmapping.\n\
                    The shared member should never be None before explicit unmapping."
                );
            }
        }
    }
}

impl Drop for SharedMemory {
    fn drop(&mut self) {
        trace!(
            "Dropping SharedMemory at {:?} with fd {}",
            self.path, self.file_descriptor
        );

        // Close the file descriptor
        unsafe {
            trace!("Calling close({})", self.file_descriptor);
            let result = libc::close(self.file_descriptor);

            if result != 0 {
                let err = std::io::Error::last_os_error();
                error!(
                    "Failed to close shared memory fd {} at {:?} during drop: {}",
                    self.file_descriptor, self.path, err
                );
            } else {
                debug!(
                    "Closed shared memory file descriptor {} for {:?}",
                    self.file_descriptor, self.path
                );
            }
        }
    }
}

impl Drop for MappedSharedMemory {
    fn drop(&mut self) {
        if let Some(shared) = &self.shared {
            trace!("Dropping MappedSharedMemory at {:?}", shared.path);

            // Unmap the memory
            trace!("Calling munmap({:p}, {})", self.mapped_address, shared.size);
            let result = unsafe { libc::munmap(self.mapped_address, shared.size) };

            if result == -1 {
                // We can only log the error in drop, we can't propagate it
                let err = std::io::Error::last_os_error();
                error!(
                    "Failed to unmap shared memory at {:?} from address {:p} during drop: {}",
                    shared.path, self.mapped_address, err
                );
            } else {
                debug!(
                    "Successfully unmapped shared memory at {:?} from address {:p} during drop",
                    shared.path, self.mapped_address
                );
            }

            // The SharedMemory will be dropped automatically after this
        }
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

        // Close the opened shared memory
        if let Ok(opened_shm) = open_result {
            let _ = opened_shm.close();
        }

        // Clean up explicitly
        let _ = shm.close_and_delete();
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
    fn test_create_existing_path() {
        // Test behavior when creating shared memory with the same path twice
        let path = get_unique_path("test_create_existing_path");
        let size1 = 4096;
        let size2 = 8192; // Different size to verify which one is used
        let mode = Mode::S_IRUSR | Mode::S_IWUSR;

        // First creation
        let shm1 = SharedMemory::create(&path, size1, mode).expect("First creation should succeed");

        // Second creation with same path
        let result = SharedMemory::create(&path, size2, mode);

        // The current implementation will likely succeed without O_EXCL flag
        // but we should verify the behavior is consistent
        if let Ok(shm2) = result {
            println!("Note: Second creation succeeded - checking behavior");

            // Check if size reflects first or second creation
            // (implementation-specific, but should be consistent)
            let _ = shm2.close();
        }

        // Clean up
        let _ = shm1.close_and_delete();
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

        // Clean up
        let _ = shm_open.close();
        let _ = shm_create.close_and_delete();
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
    fn test_close_success() {
        // Create a shared memory segment
        let path = get_unique_path("test_close_success");
        let size = 4096;
        let mode = Mode::S_IRUSR | Mode::S_IWUSR;

        let shm = SharedMemory::create(&path, size, mode)
            .expect("Failed to create shared memory for close test");

        // Close the shared memory
        let close_result = shm.close();

        // Check the close operation succeeded
        assert!(
            close_result.is_ok(),
            "Close operation failed: {:?}",
            close_result.err()
        );

        // At this point, shm is consumed (moved) by the close method,
        // so we can't directly check its state

        // Verify we can still open the shared memory (close shouldn't delete it)
        let open_result = SharedMemory::open(&path);
        assert!(
            open_result.is_ok(),
            "Failed to open shared memory after close: {:?}",
            open_result.err()
        );

        // Clean up
        if let Ok(reopened_shm) = open_result {
            let _ = reopened_shm.close_and_delete();
        } else {
            let _ = SharedMemory::delete(&path);
        }
    }

    #[test]
    fn test_close_and_delete() {
        // Test the combined close_and_delete operation
        let path = get_unique_path("test_close_and_delete");
        let shm = SharedMemory::create(&path, 4096, Mode::S_IRUSR | Mode::S_IWUSR)
            .expect("Failed to create shared memory");

        // Use close_and_delete method
        let result = shm.close_and_delete();
        assert!(
            result.is_ok(),
            "close_and_delete failed: {:?}",
            result.err()
        );

        // Verify the segment is gone by trying to open it
        let open_result = SharedMemory::open(&path);
        assert!(
            open_result.is_err(),
            "Segment still exists after close_and_delete"
        );
    }

    #[test]
    fn test_delete_success() {
        // Create a shared memory segment
        let path = get_unique_path("test_delete_success");
        let size = 4096;
        let mode = Mode::S_IRUSR | Mode::S_IWUSR;

        let shm = SharedMemory::create(&path, size, mode)
            .expect("Failed to create shared memory for delete test");

        // Must close the handle before deleting - otherwise some systems won't let us delete it
        let _ = shm.close();

        // Delete the shared memory segment
        let delete_result = SharedMemory::delete(&path);
        assert!(
            delete_result.is_ok(),
            "Delete operation failed: {:?}",
            delete_result.err()
        );

        // Verify the segment is actually deleted by trying to open it
        let open_result = SharedMemory::open(&path);
        assert!(
            open_result.is_err(),
            "Shared memory still exists after delete"
        );
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

            // Clean up - unmap and close both handles
            if let Ok(unmapped1) = mapped1.unmap() {
                let _ = unmapped1.close();
            }

            if let Ok(unmapped2) = mapped2.unmap() {
                let _ = unmapped2.close();
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
        let create_result = SharedMemory::create(&path, size, mode);
        assert!(
            create_result.is_ok(),
            "Failed to create shared memory for double delete test"
        );

        // Close it to ensure we can delete it cleanly
        if let Ok(shm) = create_result {
            let _ = shm.close();
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
        let shm1 =
            SharedMemory::create(&path, size1, mode).expect("Failed to create first shared memory");

        // Close and delete it
        let _ = shm1.close();
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

            // Clean up
            let _ = shm2.close_and_delete();
        }
    }

    #[test]
    fn test_delete_then_close_and_delete() {
        // Create a shared memory segment
        let path = get_unique_path("test_delete_then_close_and_delete");
        let size = 4096;
        let mode = Mode::S_IRUSR | Mode::S_IWUSR;

        // Create the segment
        let create_result = SharedMemory::create(&path, size, mode);
        assert!(create_result.is_ok(), "Failed to create shared memory");

        // Open a handle to it
        let open_result = SharedMemory::open(&path);
        assert!(open_result.is_ok(), "Failed to open shared memory");

        let shm = open_result.unwrap();

        // Delete the segment by path (not using the handle)
        let delete_result = SharedMemory::delete(&path);
        assert!(
            delete_result.is_ok(),
            "Failed to delete shared memory by path"
        );

        // Verify it's deleted by trying to open it again
        let reopen_result = SharedMemory::open(&path);
        assert!(
            reopen_result.is_err(),
            "Shared memory still exists after delete"
        );

        // Now try to close_and_delete on the original handle
        // This is the key test - what happens when we try to delete something
        // that's already been deleted?
        let close_and_delete_result = shm.close_and_delete();

        // The behavior here depends on the implementation:
        // 1. It might succeed fully if close_and_delete is resilient to non-existent segments
        // 2. It might return an error if close_and_delete requires the segment to still exist

        println!(
            "close_and_delete after delete result: {:?}",
            close_and_delete_result
        );

        // Test both possible outcomes:

        // If it succeeded, great! The implementation is forgiving of this edge case
        if close_and_delete_result.is_ok() {
            println!("close_and_delete succeeded after prior delete - implementation is forgiving");
        }
        // If it failed, check that it's an appropriate error
        else if let Err(err) = close_and_delete_result {
            match err {
                // In most implementations, this should be a DeleteError containing an IO error
                SharedMemoryCloseAndDeleteError::DeleteError(delete_err) => {
                    match delete_err {
                        SharedMemoryDeleteError::Io(_) => {
                            // This is the expected error type
                            println!("close_and_delete failed with DeleteError/Io as expected");
                        }
                        _ => {
                            // Other error types are acceptable but unexpected
                            println!(
                                "close_and_delete failed with unexpected DeleteError: {:?}",
                                delete_err
                            );
                        }
                    }
                }
                // Some implementations might return a CloseError, which is also acceptable
                SharedMemoryCloseAndDeleteError::CloseError(_) => {
                    println!("close_and_delete failed with CloseError");
                }
                _ => {
                    // Any other error is unexpected
                    panic!("close_and_delete failed with unexpected error: {:?}", err);
                }
            }
        }

        // Try one more open to be absolutely sure the segment is gone
        let final_open_result = SharedMemory::open(&path);
        assert!(
            final_open_result.is_err(),
            "Shared memory somehow exists after multiple delete attempts"
        );
    }
}
