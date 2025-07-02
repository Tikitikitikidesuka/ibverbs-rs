use log::{debug, error, trace};
use nix::errno::Errno;
use nix::fcntl::{Flock, FlockArg};
use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FileLockCreateError {
    #[error("IO error while creating file: {0}")]
    Io(#[from] io::Error),

    #[error("Invalid file path: {path}")]
    InvalidPath { path: PathBuf },
}

#[derive(Error, Debug)]
pub enum FileLockOpenError {
    #[error("IO error while opening file: {0}")]
    Io(#[from] io::Error),

    #[error("Invalid file path: {path}")]
    InvalidPath { path: PathBuf },
}

#[derive(Error, Debug)]
pub enum FileLockLockError {
    #[error("IO error while locking file: {0}")]
    Io(#[from] io::Error),

    #[error("No file handle available")]
    NoFileHandle,
}

#[derive(Error, Debug)]
pub enum FileLockTryLockError {
    #[error("IO error while trying to lock file: {0}")]
    Io(#[from] io::Error),

    #[error("No file handle available")]
    NoFileHandle,
}

#[derive(Error, Debug)]
pub enum FileLockUnlockError {
    #[error("IO error while unlocking file: {0}")]
    Io(#[from] io::Error),

    #[error("No lock handle available")]
    NoLockHandle,
}

#[derive(Error, Debug)]
pub enum FileLockDeleteError {
    #[error("IO error while deleting file: {0}")]
    Io(#[from] io::Error),

    #[error("Error unlocking file before deletion: {0}")]
    UnlockError(#[from] FileLockUnlockError),
}

/// A minimal file lock wrapper that provides functionality for file creation,
/// opening, locking/unlocking, and deletion.
pub struct FileLock {
    flock: Option<Flock<File>>,
    file: Option<File>,
    path: PathBuf,
}

impl FileLock {
    /// Creates a new file and returns a FileLock.
    pub fn create<P: AsRef<Path>>(path: P) -> Result<Self, FileLockCreateError> {
        let path_ref = path.as_ref();
        debug!("Creating file at path: {:?}", path_ref);

        let file = match OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path_ref)
        {
            Ok(f) => f,
            Err(e) => {
                error!("Failed to create file at {:?}: {}", path_ref, e);
                return Err(FileLockCreateError::Io(e));
            }
        };

        debug!("Successfully created file at {:?}", path_ref);
        Ok(Self {
            flock: None,
            file: Some(file),
            path: path_ref.to_path_buf(),
        })
    }

    /// Opens a file and returns a FileLock.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, FileLockOpenError> {
        let path_ref = path.as_ref();
        debug!("Opening file read-only at path: {:?}", path_ref);

        let file = match OpenOptions::new().read(true).open(path_ref) {
            Ok(f) => f,
            Err(e) => {
                error!("Failed to open file read-only at {:?}: {}", path_ref, e);
                return Err(FileLockOpenError::Io(e));
            }
        };

        debug!("Successfully opened file read-only at {:?}", path_ref);
        Ok(Self {
            flock: None,
            file: Some(file),
            path: path_ref.to_path_buf(),
        })
    }

    /// Returns the path of the file.
    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    /// Locks the file exclusively (blocking).
    pub fn lock(&mut self) -> Result<(), FileLockLockError> {
        if self.is_locked() {
            trace!("File already locked, skipping lock operation");
            return Ok(());
        }

        trace!("Attempting to lock file: {:?}", self.path);
        if let Some(file) = self.file.take() {
            match Flock::lock(file, FlockArg::LockExclusive) {
                Ok(locked_file) => {
                    debug!("Successfully locked file: {:?}", self.path);
                    self.flock = Some(locked_file);
                    Ok(())
                }
                Err((file, errno)) => {
                    self.file = Some(file);
                    error!("Failed to lock file {:?}: {}", self.path, errno);
                    Err(FileLockLockError::Io(io::Error::from_raw_os_error(
                        errno as i32,
                    )))
                }
            }
        } else {
            error!("No file handle available to lock: {:?}", self.path);
            Err(FileLockLockError::NoFileHandle)
        }
    }

    /// Tries to lock the file exclusively (non-blocking).
    ///
    /// Returns `Ok(true)` if the lock was acquired, `Ok(false)` if the file is locked
    /// by another process, or an error if something went wrong.
    pub fn try_lock(&mut self) -> Result<bool, FileLockTryLockError> {
        if self.is_locked() {
            trace!("File already locked, skipping try_lock operation");
            return Ok(true);
        }

        trace!("Attempting to try_lock file: {:?}", self.path);
        if let Some(file) = self.file.take() {
            match Flock::lock(file, FlockArg::LockExclusiveNonblock) {
                Ok(locked_file) => {
                    debug!(
                        "Successfully acquired non-blocking lock on file: {:?}",
                        self.path
                    );
                    self.flock = Some(locked_file);
                    Ok(true)
                }
                Err((file, errno)) => {
                    self.file = Some(file);
                    if errno == Errno::EAGAIN {
                        debug!("File is locked by another process: {:?}", self.path);
                        Ok(false) // File is locked by someone else
                    } else {
                        error!("Failed to try_lock file {:?}: {}", self.path, errno);
                        Err(FileLockTryLockError::Io(io::Error::from_raw_os_error(
                            errno as i32,
                        )))
                    }
                }
            }
        } else {
            error!("No file handle available to try_lock: {:?}", self.path);
            Err(FileLockTryLockError::NoFileHandle)
        }
    }

    /// Unlocks the file.
    pub fn unlock(&mut self) -> Result<(), FileLockUnlockError> {
        if !self.is_locked() {
            trace!("File not locked, skipping unlock operation");
            return Ok(());
        }

        trace!("Attempting to unlock file: {:?}", self.path);
        if let Some(locked_file) = self.flock.take() {
            match locked_file.unlock() {
                Ok(file) => {
                    debug!("Successfully unlocked file: {:?}", self.path);
                    self.file = Some(file);
                    Ok(())
                }
                Err((locked_file, errno)) => {
                    self.flock = Some(locked_file);
                    error!("Failed to unlock file {:?}: {}", self.path, errno);
                    Err(FileLockUnlockError::Io(io::Error::from_raw_os_error(
                        errno as i32,
                    )))
                }
            }
        } else {
            error!("No lock handle available to unlock: {:?}", self.path);
            Err(FileLockUnlockError::NoLockHandle)
        }
    }

    /// Deletes the file.
    ///
    /// This will first ensure the file is unlocked before deletion.
    pub fn delete(mut self) -> Result<(), FileLockDeleteError> {
        debug!("Attempting to delete file: {:?}", self.path);

        if self.is_locked() {
            trace!("File is locked, unlocking before deletion");
            self.unlock()?;
        }

        match std::fs::remove_file(&self.path) {
            Ok(_) => {
                debug!("Successfully deleted file: {:?}", self.path);
                Ok(())
            }
            Err(e) => {
                error!("Failed to delete file {:?}: {}", self.path, e);
                Err(FileLockDeleteError::Io(e))
            }
        }
    }

    /// Returns whether the file is currently locked.
    pub fn is_locked(&self) -> bool {
        self.flock.is_some()
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        trace!("Dropping FileLock for {:?}", self.path);

        // Flock automatically unlocks on drop, but we'll make it explicit
        if self.is_locked() {
            trace!("File is locked, explicitly unlocking during drop");

            if let Err(e) = self.unlock() {
                error!("Failed to unlock file {:?} during drop: {:?}", self.path, e);
            } else {
                debug!("Successfully unlocked file during drop: {:?}", self.path);
            }
        }
    }
}
