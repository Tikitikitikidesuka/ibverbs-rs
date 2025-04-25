use std::fs::{File, OpenOptions};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FileLockError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to acquire lock: {0}")]
    AcquireFailed(String),

    #[error("Failed to release lock: {0}")]
    ReleaseFailed(String),
}

pub struct FileLock {
    file: File,
    path: String,
    delete_on_drop: bool,
}

/*
impl FileLock {
    pub fn new<T: Into<String>>(path: T, delete_on_drop: bool) -> Result<Self, FileLockError> {
        let path = path.into();

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path.as_ref())?;

        Ok(Self {
            file,
            path,
            delete_on_drop,
        })
    }

    pub fn open<T: Into<String>>(path: T) -> Result<Self, FileLockError> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;

        Ok(Self {
            file,
            path: path.into(),
            delete_on_drop: false,
        })
    }
}
*/