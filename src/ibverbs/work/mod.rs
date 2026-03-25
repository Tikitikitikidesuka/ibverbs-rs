//! Work requests and completions — Send, Receive, Write, and Read operations.
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
//!
//! # Example: constructing work requests
//!
//! ```no_run
//! use ibverbs_rs::ibverbs;
//! use ibverbs_rs::ibverbs::memory::RemoteMemoryRegion;
//! use ibverbs_rs::ibverbs::work::*;
//!
//! let ctx = ibverbs::open_device("mlx5_0")?;
//! let pd = ctx.allocate_pd()?;
//!
//! let mut buf = [0u8; 128];
//! let mr = pd.register_local_mr_slice(&buf)?;
//!
//! // Two-sided: send and receive
//! let send_wr = SendWorkRequest::new(&[mr.gather_element(&buf[..64])]);
//! let recv_wr = ReceiveWorkRequest::new(&mut [mr.scatter_element(&mut buf[64..])]);
//!
//! // One-sided: RDMA write and read (requires a RemoteMemoryRegion from the peer)
//! let remote = RemoteMemoryRegion::new(0x7f000000, 128, 0xABCD);
//! let write_wr = WriteWorkRequest::new(&[mr.gather_element(&buf[..64])], remote);
//! let read_wr = ReadWorkRequest::new(&mut [mr.scatter_element(&mut buf[64..])], remote);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod completion;
mod error;
mod request;
mod success;

pub use completion::{WorkCompletion, WorkResult};
pub use error::{WorkError, WorkErrorClass, WorkErrorCode};
pub use request::{ReadWorkRequest, ReceiveWorkRequest, SendWorkRequest, WriteWorkRequest};
pub use success::WorkSuccess;
