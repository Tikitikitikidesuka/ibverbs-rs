//! Device discovery and management.
//!
//! This module provides the entry point for working with RDMA devices.
//! Before allocating resources (Protection Domains, Queue Pairs, etc.), you must
//! identify a specific RDMA device available on the system and open a [`Context`] from it.
//!
//! # Core Concepts
//!
//! * **Discovery** — Use [`list_devices`] to enumerate all available hardware, or
//!   [`open_device`] to look up a specific device by name (e.g., `"mlx5_0"`).
//! * **Device List** — The [`DeviceList`] struct owns the underlying list of devices
//!   returned by the system. It handles memory management (freeing the list when dropped).
//! * **Device Reference** — A [`Device`] is a transient handle to a specific device.
//!   It is obtained by iterating a list or querying a context.
//! * **Context** — The [`Context`] represents an active session with the hardware.
//!   It is the root factory for creating all other resources.
//!
//! # Quick Start: Open by Name
//!
//! The easiest way to get started is to open a device directly if you know its name:
//!
//! ```no_run
//! use ibverbs_rs::ibverbs;
//!
//! let ctx = ibverbs::open_device("mlx5_0")?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Example: Enumerating Devices
//!
//! If you need to inspect devices (e.g., to check GUIDs) before opening:
//!
//! ```no_run
//! use ibverbs_rs::ibverbs;
//!
//! let dev_list = ibverbs::list_devices()?;
//!
//! if dev_list.is_empty() {
//!     println!("No RDMA devices found.");
//!     return Ok(());
//! }
//!
//! for dev in dev_list.iter() {
//!     println!("Name: {:?}, GUID: {:?}", dev.name(), dev.guid());
//! }
//!
//! // Open the first available device
//! let first_dev = dev_list.get(0).unwrap();
//! let context = first_dev.open()?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod context;
mod guid;
mod manager;

pub use context::Context;
pub use guid::Guid;
pub use manager::{Device, DeviceList, DeviceListIter, list_devices, open_device};

/// Port number 1 of each HCA is the RDMA port.
pub(crate) const IB_PORT: u8 = 1;
/// Port number 2 of each HCA is the Ethernet port.
pub(crate) const _ETH_PORT: u8 = 2;
