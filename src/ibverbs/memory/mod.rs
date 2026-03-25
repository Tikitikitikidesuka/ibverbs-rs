//! Memory — registration, scatter/gather elements, and remote memory handles.
//!
//! This module provides safe abstractions for registering, accessing, and transferring data
//! through RDMA (Remote Direct Memory Access). It consists of three core components that work
//! together to enable both local and remote memory operations while maintaining Rust's safety guarantees.
//!
//! # Core Components
//!
//! ## [`MemoryRegion`]
//!
//! A registered block of local memory that the NIC can access directly via DMA. Registration
//! pins the memory (preventing OS swapping) and provides the NIC with virtual-to-physical
//! address translation.
//!
//! ## [`RemoteMemoryRegion`]
//!
//! A handle to memory on a remote peer, containing the coordinates (Address, Length, RKey)
//! needed for one-sided RDMA operations (Read/Write). Unlike local operations, remote operations
//! are strictly contiguous with no scatter/gather support.
//!
//! ## [`GatherElement`] and [`ScatterElement`]
//!
//! Scatter/Gather Elements (SGEs) serve dual purposes:
//! 1. **Data Layout**: Define how non-contiguous memory buffers are serialized/deserialized to/from the network stream
//! 2. **Safety Enforcement**: Bridge Rust's borrow checker with hardware's asynchronous operations
//!
//! # RDMA Operation Types
//!
//! RDMA supports two fundamental operation categories, each with different safety characteristics:
//!
//! ## Two-Sided Operations (Send/Receive)
//!
//! Both communicating nodes actively participate by posting Work Requests.
//!
//! * **Send** — The sender uses [`GatherElement`]s to specify local memory to read from (`&[u8]`).
//! * **Receive** — The receiver uses [`ScatterElement`]s to specify local memory to write into (`&mut [u8]`).
//! * **Safety** — Guaranteed by Rust's borrow checker. SGE creation requires valid references,
//!   ensuring memory remains alive and respects aliasing rules for the operation's duration.
//!
//! ## One-Sided Operations (RDMA Read/Write)
//!
//! Only one node actively initiates the operation; the peer's memory is accessed without its involvement.
//!
//! * **Active Side (Initiator)** — Safe. Uses local SGEs with the same guarantees as two-sided operations.
//! * **Passive Side (Target)** — **Unsafe**. The target doesn't post Work Requests, it simply registers
//!   memory with remote access permissions and waits. This breaks safety guarantees:
//!     - **No lifetime enforcement**: Remote peers can access memory that may have been deallocated
//!     - **Aliasing violations**: Remote writes can occur at any time, violating Rust's borrowing rules
//!
//! # The Safety Architecture: "Usage-Time" Guarantees
//!
//! This library enforces safety **at usage time** rather than registration time, providing flexibility
//! without sacrificing correctness.
//!
//! ## Registration vs. Usage
//!
//! **Registration** (creating a [`MemoryRegion`]) does not require owning the buffer. This allows:
//! * Registering the same buffer in multiple [`ProtectionDomain`](crate::ibverbs::protection_domain::ProtectionDomain)s.
//! * Registering memory owned by other structures.
//! * Flexible memory management patterns.
//!
//! **Usage** (creating SGEs and posting Work Requests) is where safety is enforced:
//! *   [`GatherElement::new`] requires `&[u8]` → proves data is alive and immutable
//! *   [`ScatterElement::new`] requires `&mut [u8]` → proves exclusive access
//! *   SGE lifetimes bind to the data, preventing use-after-free
//!
//! ## Access Permissions and Safety
//!
//! Memory regions can be registered with different access levels:
//!
//! ### Safe Registration
//!
//! * [`MemoryRegion::register_local_mr`] — **Safe**. Sets only local write access.
//!   - Allows: Send, Receive, and acting as initiator in RDMA Read/Write.
//!   - Safety: All operations require SGE creation, enforcing Rust's borrowing rules.
//!
//! ### Unsafe Registration
//!
//! * [`MemoryRegion::register_shared_mr`] — **Unsafe**. Adds remote read and remote write access.
//!   - Allows: Being the target of remote RDMA operations.
//!   - Risk: Remote peers can access memory at any time, breaking aliasing guarantees.
//!   - Responsibility: You must manually ensure memory stays alive and is not aliased
//!     locally while remote operations are executed.
//!
//! * [`MemoryRegion::register_mr_with_access`] — **Unsafe**. Full manual control.
//!
//! # Data Layout: Scatter and Gather
//!
//! The network transmission is a continuous **stream of bytes** where buffer boundaries are lost.
//!
//! ## Outgoing: "Gather" (Serialization)
//!
//! ```text
//! Local Memory:           Network Stream:
//! ┌─────────┐
//! │ A A A A │ ───────┐
//! └─────────┘        │     ┌─────────────────────────┐
//! ┌─────────────┐    ├───▶ │ A A A A B B B B B B C C │
//! │ B B B B B B │ ───┤     └─────────────────────────┘
//! └─────────────┘    │
//! ┌─────┐            │
//! │ C C │ ───────────┘
//! └─────┘
//! ```
//!
//! The NIC "gathers" data from multiple [`GatherElement`]s into a single continuous stream.
//!
//! ## Incoming: "Scatter" (Deserialization)
//!
//! The NIC "scatters" the incoming stream across multiple [`ScatterElement`]s, filling each
//! buffer sequentially until the stream is exhausted.
//!
//! ### Matching Scatter Layout
//!
//! ```text
//! Network Stream:                 Local Memory:
//!
//!                                          ┌─────────┐
//!                                ┌───────▶ │ A A A A │
//! ┌─────────────────────────┐    │         └─────────┘
//! │ A A A A B B B B B B C C │ ───┤     ┌─────────────┐
//! └─────────────────────────┘    ├───▶ │ B B B B B B │
//!                                │     └─────────────┘
//!                                │             ┌─────┐
//!                                └───────────▶ │ C C │
//!                                              └─────┘
//! ```
//!
//! ### Non-Matching Scatter Layout
//!
//! **The sender's gather list and receiver's scatter list do NOT need to match.**
//! Only the total byte length must be equal. The stream is simply cut differently:
//!
//! ```text
//! Non matching scatter elements
//! Network Stream:                Local Memory:
//!
//! ┌─────────────────────────┐            ┌───────────┐
//! │ A A A A B B B B B B C C │ ──┬──────▶ │ A A A A B │
//! └─────────────────────────┘   │        └───────────┘
//!                               │    ┌───────────────┐
//!                               └──▶ │ B B B B B C C │
//!                                    └───────────────┘
//! ```
//!
//! In this example, the sender used 3 gather elements (4 + 6 + 2 = 12 bytes), while the
//! receiver used 2 scatter elements (5 + 7 = 12 bytes). The data arrives intact, just
//! partitioned differently in memory.
//!
//! # Remote Memory Safety: The Unavoidable UB Risk
//!
//! [`RemoteMemoryRegion`] represents a fundamental safety limitation in RDMA:
//!
//! ## Why Remote Operations Are Unsafe
//!
//! In local operations, we enforce safety by tying buffer lifetimes to SGEs. **This is impossible
//! for remote memory** because:
//! * The memory resides on a different machine.
//! * No local knowledge of remote buffer lifecycle exists.
//! * No mechanism to verify remote memory validity.
//!
//! ## Safety Boundaries
//!
//! * **Local Safety** — Safe. Invalid remote addresses cause operation failures, not local memory corruption.
//! * **Remote Safety** — Unsafe. Writing to deallocated remote memory causes use-after-free on the remote peer.
//!
//! ## Responsibility Model
//!
//! When you call `register_shared_mr` (unsafe), you accept responsibility for:
//! 1.  **Lifetime management**: Keep memory alive while peers hold valid RKeys
//! 2.  **Coordination**: Establish protocols to invalidate/revoke remote access before deallocation
//! 3.  **Aliasing**: Avoid local access patterns that conflict with concurrent remote writes
//!
//! # Example: registering memory and creating SGEs
//!
//! ```no_run
//! use ibverbs_rs::ibverbs;
//! use ibverbs_rs::ibverbs::work::{SendWorkRequest, ReceiveWorkRequest};
//!
//! let ctx = ibverbs::open_device("mlx5_0")?;
//! let pd = ctx.allocate_pd()?;
//!
//! // Register a buffer as a local memory region
//! let mut buf = vec![0u8; 4096];
//! let mr = pd.register_local_mr_slice(&buf)?;
//!
//! // Create a GatherElement for sending (borrows &[u8])
//! let gather = mr.gather_element(&buf[..512]);
//! let _send_wr = SendWorkRequest::new(&[gather]);
//!
//! // Create a ScatterElement for receiving (borrows &mut [u8])
//! let scatter = mr.scatter_element(&mut buf[512..1024]);
//! let _recv_wr = ReceiveWorkRequest::new(&mut [scatter]);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Remote Memory Navigation
//!
//! Helper macros for working with structured remote data:
//!
//! * [`remote_array_field!`] — Access N-th element of remote array.
//! * [`remote_struct_field!`] — Access specific field of remote struct.
//! * [`remote_struct_array_field!`] — Access field within array element.
//! * `*_unchecked` variants — Skip client-side bounds checking.

mod memory_region;
mod remote_memory_region;
mod scatter_gather_element;

pub use memory_region::MemoryRegion;
pub use remote_memory_region::RemoteMemoryRegion;
pub use scatter_gather_element::{GatherElement, ScatterElement, ScatterGatherElementError};

pub use crate::remote_array_field;
pub use crate::remote_array_field_unchecked;
pub use crate::remote_struct_array_field;
pub use crate::remote_struct_array_field_unchecked;
pub use crate::remote_struct_field;
pub use crate::remote_struct_field_unchecked;
