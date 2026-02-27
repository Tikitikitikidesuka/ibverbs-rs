use log::warn;
use nix::errno::Errno;
use nix::libc;
use nix::libc::{F_TLOCK, F_ULOCK};
use std::fs::{File, OpenOptions};
use std::io;
use std::io::Seek;
use std::os::fd::AsRawFd;
use std::path::Path;

/// POSIX advisory whole-file lock.
///
/// This type wraps a `File` opened from a path and holds a POSIX advisory,
/// whole-file lock on it using `lockf`. It is intended to be used as a guard
/// for producer–consumer exclusivity of the `SharedMemoryBuffer`.
/// Functionality is limited to acquiring a **non-blocking** advisory lock on
/// the whole file and releasing it when the guard is dropped.
///
/// While an instance of this struct exists, the underlying file descriptor is
/// locked using `lockf` in a *cooperative* (advisory) manner. Other processes
/// (or other code in the same process) must also use POSIX advisory locking
/// and honor the convention for this lock to be effective; the kernel does
/// not enforce it for arbitrary I/O.
///
/// This lock is:
/// - Advisory (cooperative) rather than mandatory.
/// - Whole-file: it covers the entire file from offset 0 to EOF for the
///   intended lifetime of the guard.
/// - Automatically released (best effort) when `PosixAdvisoryFileLock` is
///   dropped
///
/// This API is Unix-specific and relies on `libc::lockf`.
///
/// ## Lock semantics
///
/// - Calling `PosixAdvisoryFileLock::try_lock` opens the path with the provided
///   `OpenOptions`, creating a new kernel *open file description* with its own
///   advisory lock state.
/// - Opening the same path again elsewhere (e.g., with `File::open` or another
///   `OpenOptions::open`) creates a separate open file description, which can
///   independently attempt to acquire or contend for an advisory lock.
/// - As a result, two callers using the same file path (even in the same
///   process) can contend on this lock just like two different processes, as
///   long as they both use POSIX advisory locking.
///
/// ## Example
///
/// ```no-run
/// # use std::fs::OpenOptions;
/// # use std::io::{self, Write};
/// # use shared_memory_buffer::posix_advisory_file_lock::AdvisoryFileLock;
/// #
///  const PATH: &str = "/tmp/lockfile";
///
///  { // Try to acquire a non-blocking advisory lock on the whole file.
///     let _lock = AdvisoryFileLock::try_lock(
///         &PATH,
///         &OpenOptions::new()
///             .create(true)
///             .read(true)
///             .write(true),
///     )?;
///
///     println!("Critical section");
///  } // Lock automatically released when `_lock` is dropped at end of scope.
/// ```
#[derive(Debug)]
pub struct AdvisoryFileLock {
    file: File,
}

impl AdvisoryFileLock {
    /// Try to acquire an advisory whole-file lock.
    ///
    /// This function:
    /// - Opens the file at `path` using the provided `open_options`, creating a
    ///   new kernel open file description with its own advisory lock state.
    /// - Seeks the file cursor to offset 0 before attempting the lock.
    /// - Uses `lockf(fd, F_TLOCK, 0)` to request a non-blocking, whole-file
    ///   advisory lock, where a length of `0` means "from the current offset
    ///   to EOF".
    ///
    /// Returns:
    /// - `Ok(PosixAdvisoryFileLock)` if the lock was acquired and the file is
    ///   now owned by the returned guard.
    /// - `Err(e)` with `e.kind() == io::ErrorKind::WouldBlock` if the file is
    ///   already locked by another cooperating process or a conflicting lock
    ///   in this process.
    /// - `Err(e)` for any other underlying OS error.
    pub fn try_lock(
        path: impl AsRef<Path>,
        open_options: &OpenOptions,
    ) -> io::Result<AdvisoryFileLock> {
        let mut file = open_options.open(path)?;
        file.seek(io::SeekFrom::Start(0))?;

        let lockf_result = unsafe { libc::lockf(file.as_raw_fd(), F_TLOCK, 0) };
        if lockf_result == -1 {
            let errno = Errno::last() as i32;
            Err(if errno == libc::EACCES || errno == libc::EAGAIN {
                io::Error::new(io::ErrorKind::WouldBlock, "file is already locked")
            } else {
                io::Error::from_raw_os_error(errno)
            })
        } else {
            Ok(Self { file })
        }
    }
}

impl Drop for AdvisoryFileLock {
    /// Release the advisory lock on drop.
    ///
    /// This is a best-effort unlock.
    /// Any failure to seek or unlock is logged via `warn!`.
    ///
    /// Note: The kernel automatically releases `lockf()` locks when the last
    /// file descriptor referencing the open file description is closed. This
    /// explicit unlock ensures the lock is released as soon as the guard is
    /// dropped, rather than waiting for the `File` to be closed implicitly.
    fn drop(&mut self) {
        // Another handle might have moved the offset, so seek to 0 before unlocking
        if let Err(error) = self.file.seek(io::SeekFrom::Start(0)) {
            warn!(
                "PosixAdvisoryFileLockGuard: Failed to seek to start of file on drop (error={error:?})"
            );
        }

        let lockf_result = unsafe { libc::lockf(self.file.as_raw_fd(), F_ULOCK, 0) };
        if lockf_result == -1 {
            let errno = Errno::last();
            warn!("PosixAdvisoryFileLockGuard: Failed to unlock file on drop (errno={errno})");
        }
    }
}
