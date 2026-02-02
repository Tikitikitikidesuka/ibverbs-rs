use nix::libc;
use std::io;
use thiserror::Error;

pub type IbvResult<T> = Result<T, IbvError>;

#[derive(Debug, Error)]
pub enum IbvError {
    /// Maps to `EINVAL`.
    #[error("invalid argument: {0}")]
    InvalidInput(String),

    /// Maps to `ENOMEM`, `EMFILE`, `EAGAIN`.
    #[error("resource exhausted: {0}")]
    Resource(String),

    /// Maps to `EPERM`, `EACCES`.
    #[error("permission denied: {0}")]
    Permission(String),

    /// Maps to `ENOENT`.
    #[error("entity not found: {0}")]
    NotFound(String),

    /// Catch-all for underlying OS or Driver failures that don't fit other categories.
    /// This wraps the standard `std::io::Error`.
    #[error("driver/OS error: {0}")]
    Driver(#[from] io::Error),
}

impl IbvError {
    /// Helper to convert a raw `errno` (captured via `io::Error`) into a semantic `IbvError`.
    pub(crate) fn from_errno_with_msg(errno: i32, msg: impl Into<String>) -> Self {
        match errno {
            libc::EINVAL => {
                IbvError::InvalidInput(format!("{} (driver rejected params)", msg.into()))
            }
            libc::ENOMEM => IbvError::Resource(format!("{} (out of memory)", msg.into())),
            libc::EMFILE => {
                IbvError::Resource(format!("{} (too many open files/objects)", msg.into()))
            }
            libc::EAGAIN => {
                IbvError::Resource(format!("{} (temporary resource shortage)", msg.into()))
            }
            libc::EACCES | libc::EPERM => IbvError::Permission(msg.into()),
            libc::ENOENT => IbvError::NotFound(msg.into()),
            _ => IbvError::Driver(io::Error::from_raw_os_error(errno)),
        }
    }
}
