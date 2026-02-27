use log::warn;
use nix::errno::Errno;
use nix::libc;
use nix::libc::{MAP_FAILED, c_int, off_t};
use nix::sys::stat::Mode;
use std::collections::Bound;
use std::ffi::CString;
use std::io;
use std::io::ErrorKind;
use std::ops::RangeBounds;

/// POSIX shared memory object opened via `shm_open`.
///
/// This type represents an open POSIX shared memory object identified by a
/// name and backed by a file descriptor. It does **not** itself map the
/// memory into the process; see [`MappedSharedMemory`] for that.
///
/// The underlying file descriptor is closed automatically when
/// [`SharedMemory`] is dropped.
///
/// ## Example
///
/// ```no-run
/// # use nix::sys::stat::Mode;
/// # use std::io;
/// # use shared_memory_buffer::posix_shared_memory::{
/// #     AccessMode, SharedMemory, MappedSharedMemory,
/// # };
/// #
///  const SIZE: usize = 4096;
///
///  // Create a new shared memory object with 4096 bytes, read/write access,
///  // and user read/write permissions.
///  let shm = SharedMemory::create(
///     "/my_shm".to_owned(),
///     SIZE,
///     AccessMode::ReadWrite,
///     Mode::S_IRUSR | Mode::S_IWUSR,
///  ).unwrap();
///
///  // Map shared memory into the process.
///  let mut mapped_shm = unsafe { shm.map(AccessMode::ReadWrite).unwrap() };
///
///  // Fill the mapped region with zeros.
///  unsafe { mapped_shm.as_mut_slice()[..SIZE].copy_from_slice(vec![0; SIZE].as_slice()) };
///
///  // Unlink the shared memory name so no new open() calls can find it.
///  // Existing mappings and descriptors remain valid until closed/dropped.
///  SharedMemory::unlink("/my_shm".to_owned()).unwrap();
/// ```
#[derive(Debug)]
pub struct SharedMemory {
    name: String,
    fd: c_int,
}

/// POSIX shared memory region mapped into the process via `mmap`.
///
/// This type represents a mapping of a POSIX shared memory object into the
/// process’ address space. It owns the mapping and will automatically
/// unmap it on drop using `munmap`.
///
/// Access to the contents is provided via unsafe slice accessors
/// [`MappedSharedMemory::as_slice`] and
/// [`MappedSharedMemory::as_mut_slice`].
#[derive(Debug)]
pub struct MappedSharedMemory {
    name: String,
    size: usize,
    address: *mut u8,
}

/// Access mode for POSIX shared memory.
///
/// Controls how the shared memory object is opened and, for mappings, what
/// protections are requested.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum AccessMode {
    /// Read-only access.
    ReadOnly,
    /// Write-only access.
    WriteOnly,
    /// Read and write access.
    ReadWrite,
}

impl SharedMemory {
    /// Returns the name of the shared memory.
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    /// Create a new POSIX shared memory object and open it.
    ///
    /// This function:
    /// - Creates a new shared memory object with the given `name`, `size`,
    ///   `access` mode, and `permissions` using `shm_open` and `ftruncate`.
    /// - Fails if the shared memory object already exists.
    ///
    /// Returns:
    /// - `Ok(PosixSharedMemory)` if the object was created and opened.
    /// - `Err(e)` with `e.kind() == ErrorKind::AlreadyExists` if the shared
    ///   memory object already exists.
    /// - `Err(e)` for any other underlying OS error.
    pub fn create(
        name: impl Into<String>,
        min_size: usize,
        access: AccessMode,
        permissions: Mode,
    ) -> io::Result<Self> {
        let name = name.into();

        // Convert name to CString
        let cstr_name = CString::new(name.as_bytes())
            .map_err(|_| io::Error::new(ErrorKind::InvalidInput, "name contains null byte"))?;

        // Choose open flags
        let flags = libc::O_CREAT
            | match access {
                AccessMode::ReadOnly => libc::O_RDONLY,
                AccessMode::WriteOnly => libc::O_WRONLY,
                AccessMode::ReadWrite => libc::O_RDWR,
            };

        // Attempt to open
        let shm_open_result =
            unsafe { libc::shm_open(cstr_name.as_ptr(), flags, permissions.bits()) };
        let fd = if shm_open_result == -1 {
            let errno = Errno::last() as i32;
            return Err(io::Error::from_raw_os_error(errno));
        } else {
            shm_open_result
        };

        // Truncate to the requested size
        let ftruncate_result = unsafe { libc::ftruncate(fd, min_size as off_t) };
        if ftruncate_result == -1 {
            unsafe { libc::close(fd) };
            let errno = Errno::last() as i32;
            return Err(io::Error::from_raw_os_error(errno));
        }

        Ok(SharedMemory { name, fd })
    }

    /// Open an existing POSIX shared memory object with the given access mode.
    ///
    /// This function:
    /// - Opens an already-created shared memory object with the given `name`
    ///   and `access` mode using `shm_open`.
    ///
    /// Returns:
    /// - `Ok(PosixSharedMemory)` if the object was opened successfully.
    /// - `Err(e)` with `e.kind() == ErrorKind::NotFound` if the shared
    ///   memory object does not exist.
    /// - `Err(e)` for any other underlying OS error.
    pub fn open(name: impl Into<String>, access: AccessMode) -> io::Result<Self> {
        let name = name.into();

        // Convert name to CString
        let cstr_name = CString::new(name.as_bytes())
            .map_err(|_| io::Error::new(ErrorKind::InvalidInput, "name contains null byte"))?;

        // Select flags based on access mode
        let flags = match access {
            AccessMode::ReadOnly => libc::O_RDONLY,
            AccessMode::WriteOnly => libc::O_WRONLY,
            AccessMode::ReadWrite => libc::O_RDWR,
        };

        // Open shared memory object
        let shm_open_result = unsafe { libc::shm_open(cstr_name.as_ptr(), flags, 0) };
        let fd = if shm_open_result == -1 {
            let errno = Errno::last() as i32;
            if errno == libc::ENOENT {
                return Err(io::Error::new(
                    ErrorKind::NotFound,
                    "shared memory does not exist",
                ));
            }
            return Err(io::Error::from_raw_os_error(errno));
        } else {
            shm_open_result
        };

        Ok(SharedMemory { name, fd })
    }

    /// Unlink a POSIX shared memory object by name.
    ///
    /// This removes the name from the system using `shm_unlink`, preventing
    /// further calls to [`SharedMemory::open`] with the same name from
    /// succeeding. The underlying memory object and its mappings remain
    /// valid until all file descriptors referring to it are closed.
    ///
    /// Returns:
    /// - `Ok(())` if the shared memory name was unlinked.
    /// - `Err(e)` with `e.kind() == ErrorKind::NotFound` if no such shared
    ///   memory object exists.
    /// - `Err(e)` for any other underlying OS error.
    pub fn unlink(name: impl Into<String>) -> io::Result<()> {
        let name = name.into();

        // Convert name to CString
        let cstr_name = CString::new(name.as_bytes())
            .map_err(|_| io::Error::new(ErrorKind::InvalidInput, "name contains null byte"))?;

        // Unlink shared memory object
        let shm_unlink_result = unsafe { libc::shm_unlink(cstr_name.as_ptr()) };
        if shm_unlink_result == -1 {
            let errno = Errno::last() as i32;
            if errno == libc::ENOENT {
                return Err(io::Error::new(
                    ErrorKind::NotFound,
                    "shared memory does not exist",
                ));
            }
            return Err(io::Error::from_raw_os_error(errno));
        };

        Ok(())
    }

    /// Map the shared memory object into the process address space.
    ///
    /// This function:
    /// - Obtains the current size of the shared memory object via `fstat`.
    /// - Calls `mmap` with `MAP_SHARED` and protections derived from `access`.
    ///
    /// Returns:
    /// - `Ok(MappedPosixSharedMemory)` if the mapping succeeds.
    /// - `Err(e)` for any underlying OS error (e.g. `mmap` failure).
    ///
    /// # Safety
    ///
    /// - The underlying shared memory object may be modified at any time by
    ///   other processes or threads.
    /// - While a mapping exists, the caller must ensure the shared memory
    ///   object is **not shrunk** (e.g. via `ftruncate` on the same object),
    ///   as this may invalidate the mapping and result in undefined behavior.
    /// - The returned [`MappedSharedMemory`] allows creation of slices
    ///   that must obey Rust’s aliasing and mutability rules.
    pub unsafe fn map(self, access: AccessMode) -> io::Result<MappedSharedMemory> {
        // Select flags based on access mode
        let flags = match access {
            AccessMode::ReadOnly => libc::PROT_READ,
            AccessMode::WriteOnly => libc::PROT_WRITE,
            AccessMode::ReadWrite => libc::PROT_READ | libc::PROT_WRITE,
        };

        // Get size of shared memory
        let size = self.size()?;

        // Map shared memory
        let mmap_result = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                size,
                flags,
                libc::MAP_SHARED,
                self.fd,
                0,
            )
        };
        let address = if mmap_result == MAP_FAILED {
            let errno = Errno::last() as i32;
            return Err(io::Error::from_raw_os_error(errno));
        } else {
            mmap_result
        };

        Ok(MappedSharedMemory {
            name: self.name.clone(),
            size,
            address: address as *mut u8,
        })
    }

    /// Returns the current size of the shared memory object in bytes.
    ///
    /// This queries the size via `fstat` on the underlying file descriptor.
    ///
    /// Returns:
    /// - `Ok(size)` on success.
    /// - `Err(e)` with `e.kind() == ErrorKind::NotFound` if the descriptor no
    ///   longer refers to a valid shared memory object.
    /// - `Err(e)` for any other underlying OS error.
    pub fn size(&self) -> io::Result<usize> {
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        let fstat_result = unsafe { libc::fstat(self.fd, &mut stat) };
        if fstat_result == -1 {
            let errno = Errno::last() as i32;
            if errno == libc::ENOENT {
                return Err(io::Error::new(
                    ErrorKind::NotFound,
                    "shared memory does not exist",
                ));
            }
            Err(io::Error::from_raw_os_error(errno))
        } else {
            Ok(stat.st_size as usize)
        }
    }
}

impl MappedSharedMemory {
    pub fn len(&self) -> usize {
        self.size
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.address
    }

    pub fn as_mut_ptr(&self) -> *mut u8 {
        self.address
    }

    /// Returns a shared slice for the given range if within bounds.
    ///
    /// # Safety
    ///
    /// Caller must ensure that the memory is valid and not concurrently mutably aliased.
    pub unsafe fn slice(&self, range: impl RangeBounds<usize>) -> Option<&[u8]> {
        let start = match range.start_bound() {
            Bound::Included(&n) => n,
            Bound::Excluded(&n) => n.checked_add(1)?,
            Bound::Unbounded => 0,
        };

        let end = match range.end_bound() {
            Bound::Included(&n) => n.checked_add(1)?,
            Bound::Excluded(&n) => n,
            Bound::Unbounded => self.size,
        };

        // Check validity
        if start > end || end > self.size {
            return None;
        }

        unsafe {
            Some(std::slice::from_raw_parts(
                self.address.add(start),
                end - start,
            ))
        }
    }

    /// Returns a mutable slice for the given range if within bounds.
    ///
    /// # Safety
    ///
    /// Caller must ensure that the memory is valid and not concurrently aliased.
    pub unsafe fn slice_mut(&mut self, range: impl RangeBounds<usize>) -> Option<&mut [u8]> {
        let start = match range.start_bound() {
            Bound::Included(&n) => n,
            Bound::Excluded(&n) => n.checked_add(1)?,
            Bound::Unbounded => 0,
        };

        let end = match range.end_bound() {
            Bound::Included(&n) => n.checked_add(1)?,
            Bound::Excluded(&n) => n,
            Bound::Unbounded => self.size,
        };

        if start > end || end > self.size {
            return None;
        }

        unsafe {
            Some(std::slice::from_raw_parts_mut(
                self.address.add(start),
                end - start,
            ))
        }
    }
}

impl Drop for SharedMemory {
    /// Close the shared memory object file descriptor on drop.
    ///
    /// This is a best-effort close.
    /// Any failure to close is logged via `warn!`.
    fn drop(&mut self) {
        let close_result = unsafe { libc::close(self.fd) };
        if close_result == -1 {
            let errno = Errno::last();
            warn!("PosixSharedMemory: Failed to close shared memory on drop (errno={errno})");
        }
    }
}

impl Drop for MappedSharedMemory {
    /// Unmap the shared memory region on drop.
    ///
    /// This is a best-effort unmap.
    /// Any failure to unmap is logged via `warn!`.
    fn drop(&mut self) {
        let munmap_result = unsafe { libc::munmap(self.address as *mut libc::c_void, self.size) };
        if munmap_result == -1 {
            let errno = Errno::last();
            warn!("MappedPosixSharedMemory: Failed to unmap shared memory on drop (errno={errno})");
        }
    }
}
