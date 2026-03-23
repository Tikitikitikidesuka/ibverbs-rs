//! Work Request and Completion types.
//!
//! This module defines the data structures used to represent RDMA operations and their results.
//!
//! # Work Requests
//!
//! * [`SendWorkRequest`] — Send data to a remote peer.
//! * [`ReceiveWorkRequest`] — Provide a buffer to receive incoming data.
//! * [`WriteWorkRequest`] — Write data directly to remote memory.
//! * [`ReadWorkRequest`] — Read data directly from remote memory.
//!
//! # Completions
//!
//! * [`WorkCompletion`] — The completion event returned when polling a Completion Queue.
//! * [`WorkSuccess`] — Metadata for successful operations (bytes transferred, immediate data).
//! * [`WorkError`] — Diagnostics for failed operations (error code, vendor syndrome, hints).

mod completion;
mod error;
mod request;
mod success;

pub use completion::{WorkCompletion, WorkResult};
pub use error::{WorkError, WorkErrorClass, WorkErrorCode};
pub use request::{ReadWorkRequest, ReceiveWorkRequest, SendWorkRequest, WriteWorkRequest};
pub use success::WorkSuccess;
