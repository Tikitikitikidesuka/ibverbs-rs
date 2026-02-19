use nix::errno::Errno;
use nix::fcntl::{Flock, FlockArg};
use std::fs::{File, OpenOptions};
use std::path::Path;
use thiserror::Error;
use tracing::{debug, instrument, warn};

#[derive(Debug, Error)]
pub enum LockFileError {
    #[error("IO error locking file: {0}")]
    Io(#[from] std::io::Error),

    #[error("File lock already acquired")]
    LockFileAlreadyAcquired,
}

pub struct LockFile {
    _file: Flock<File>,
}

impl LockFile {
    /// Locks the file exclusively blocking
    #[instrument(skip_all, fields(path = ?path.as_ref().display()))]
    pub fn lock<P: AsRef<Path>>(path: P) -> Result<Self, LockFileError> {
        Self::lock_helper(path, FlockArg::LockExclusive)
    }

    /// Locks the file exclusively non-blocking
    #[instrument(skip_all, fields(path = ?path.as_ref().display()))]
    pub fn try_lock<P: AsRef<Path>>(path: P) -> Result<Self, LockFileError> {
        Self::lock_helper(path, FlockArg::LockExclusiveNonblock)
    }

    fn lock_helper<P: AsRef<Path>>(path: P, flockarg: FlockArg) -> Result<Self, LockFileError> {
        debug!("Locking file");

        debug!("Checking if file exists");
        let file = if path.as_ref().exists() {
            debug!("File exists. Opening file");
            Self::open(path.as_ref()).inspect_err(|_error| {
                warn!("Failed to open file");
            })?
        } else {
            debug!("File does not exist. Creating file");
            Self::create(path.as_ref()).inspect_err(|_error| {
                warn!("Failed to create file");
            })?
        };

        debug!("Locking file. Blocking until done...");
        match Flock::lock(file, flockarg) {
            Ok(locked_file) => {
                debug!("File lock acquired");
                Ok(Self { _file: locked_file })
            }
            Err((_, errno)) => {
                if errno == Errno::EAGAIN {
                    warn!(
                        "Failed to acquire file lock. It has already been acquired by another process"
                    );
                    Err(LockFileError::LockFileAlreadyAcquired)
                } else {
                    warn!("Failed to acquire file lock");
                    Err(LockFileError::Io(std::io::Error::from_raw_os_error(
                        errno as i32,
                    )))
                }
            }
        }
    }

    fn create(path: &Path) -> Result<File, std::io::Error> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
    }

    fn open(path: &Path) -> Result<File, std::io::Error> {
        OpenOptions::new().read(true).open(path)
    }

    pub fn delete(path: &Path) -> Result<(), std::io::Error> {
        std::fs::remove_file(path)
    }
}
