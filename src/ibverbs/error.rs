//! Error types — ibverbs operation failure variants.

use nix::libc;
use std::io;
use thiserror::Error;

/// A specialized result type for ibverbs operations.
pub type IbvResult<T> = Result<T, IbvError>;

/// Represents errors that can occur when interacting with the RDMA subsystem.
///
/// This enum maps low-level OS/Driver error codes (`errno`) into high-level semantic categories
/// to help applications decide how to recover (e.g., retrying on resource exhaustion vs. panicking on invalid input).
#[derive(Debug, Error)]
pub enum IbvError {
    /// Maps to `EINVAL`.
    #[error("Invalid argument: {0}")]
    InvalidInput(String),

    /// Maps to `ENOMEM`, `EMFILE`, `EAGAIN`.
    #[error("Resource exhausted: {0}")]
    Resource(String),

    /// Maps to `EPERM`, `EACCES`.
    #[error("Permission denied: {0}")]
    Permission(String),

    /// Maps to `ENOENT`.
    #[error("Entity not found: {0}")]
    NotFound(String),

    /// Catch-all for underlying OS or Driver failures that don't fit other categories.
    /// This wraps the standard `std::io::Error`.
    #[error("Driver/OS error: {0}")]
    Driver(io::Error),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn einval_maps_to_invalid_input() {
        assert!(matches!(
            IbvError::from_errno_with_msg(libc::EINVAL, "test"),
            IbvError::InvalidInput(_)
        ));
    }

    #[test]
    fn enomem_maps_to_resource() {
        assert!(matches!(
            IbvError::from_errno_with_msg(libc::ENOMEM, "test"),
            IbvError::Resource(_)
        ));
    }

    #[test]
    fn emfile_maps_to_resource() {
        assert!(matches!(
            IbvError::from_errno_with_msg(libc::EMFILE, "test"),
            IbvError::Resource(_)
        ));
    }

    #[test]
    fn eagain_maps_to_resource() {
        assert!(matches!(
            IbvError::from_errno_with_msg(libc::EAGAIN, "test"),
            IbvError::Resource(_)
        ));
    }

    #[test]
    fn eacces_maps_to_permission() {
        let err = IbvError::from_errno_with_msg(libc::EACCES, "test");
        assert!(matches!(err, IbvError::Permission(_)));
    }

    #[test]
    fn eperm_maps_to_permission() {
        let err = IbvError::from_errno_with_msg(libc::EPERM, "test");
        assert!(matches!(err, IbvError::Permission(_)));
    }

    #[test]
    fn enoent_maps_to_not_found() {
        let err = IbvError::from_errno_with_msg(libc::ENOENT, "test");
        assert!(matches!(err, IbvError::NotFound(_)));
    }

    #[test]
    fn unknown_errno_maps_to_driver() {
        let err = IbvError::from_errno_with_msg(libc::EIO, "test");
        assert!(matches!(err, IbvError::Driver(_)));
    }
}
