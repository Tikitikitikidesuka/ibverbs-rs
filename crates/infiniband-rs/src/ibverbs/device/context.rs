use crate::ibverbs::completion_queue::CompletionQueue;
use crate::ibverbs::device::{DeviceRef, IB_PORT};
use crate::ibverbs::error::{IbvError, IbvResult};
use crate::ibverbs::protection_domain::ProtectionDomain;
use ibverbs_sys::*;
use std::io;
use std::sync::Arc;

/// A handle to an open RDMA device context.
///
/// The `Context` represents an active user-space session with a specific RDMA device.
/// It serves as the root factory for creating all other RDMA resources.
///
/// # Resource Management & Shared Ownership
///
/// The `Context` uses **shared ownership** (via [`Arc`]) to manage the underlying device connection.
/// This design simplifies resource management by allowing multiple handles to the same hardware context.
///
/// All resources created from a `Context` (such as [`ProtectionDomain`], [`CompletionQueue`], etc.)
/// implicitly hold a clone of this `Arc`. This creates a robust ownership hierarchy:
///
/// 1.  **Child Keeps Parent Alive**: Even if you drop your main `Context` handle, the underlying
///     hardware connection remains open as long as *any* child resource (PD, QP, MR) is still alive.
/// 2.  **Automatic Cleanup**: The actual `ibv_close_device` call only happens when the *last*
///     reference to the context is dropped.
///
/// # Example: The Resource Lifecycle
///
/// ```no_run
/// # use infiniband_rs::ibverbs::devices::open_device;
/// # use infiniband_rs::ibverbs::error::IbvResult;
/// # fn main() -> IbvResult<()> {
/// // 1. Open the context
/// let context = open_device("mlx5_0")?;
///
/// // 2. Create resources (PD and CQ)
/// // These resources now hold a reference to the context internally.
/// let pd = context.allocate_pd()?;
/// let cq = context.create_cq(0, 16)?;
///
/// // 3. Drop the context explicitly (optional)
/// // The device connection remains OPEN because 'pd' and 'cq' are still alive.
/// drop(context);
///
/// // 4. End of main: 'pd' and 'cq' are dropped, ref count hits zero, context closes.
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct Context {
    pub(crate) inner: Arc<ContextInner>,
}

impl Context {
    /// Creates a Completion Queue (CQ) on this device.
    ///
    /// The CQ is used to receive completion notifications for work requests posted to Queue Pairs.
    /// The returned [`CompletionQueue`] will hold a clone of this `Context`, keeping the
    /// device connection alive.
    ///
    /// # Arguments
    ///
    /// *   `min_cq_entries`: The *minimum* number of entries the CQ must support.
    ///     The hardware may allocate a larger queue.
    ///
    /// # Errors
    ///
    /// *   Returns [`IbvError::InvalidInput`] if `min_cq_entries` exceeds the device's capabilities.
    /// *   Returns [`IbvError::Resource`] if the system cannot allocate the queue resources (e.g., out of memory).
    pub fn create_cq(&self, min_cq_entries: u32) -> IbvResult<CompletionQueue> {
        CompletionQueue::create(self, min_cq_entries)
    }

    /// Allocates a Protection Domain (PD) for this context.
    ///
    /// A PD is a container for grouping Queue Pairs and Memory Regions.
    /// The returned [`ProtectionDomain`] will hold a strong reference to this `Context`,
    /// ensuring the underlying device connection remains open even if the original
    /// `Context` handle is dropped.
    ///
    /// # Errors
    ///
    /// *   Returns [`IbvError::Resource`] if the PD limit for the device has been reached or if memory allocation fails.
    pub fn allocate_pd(&self) -> IbvResult<ProtectionDomain> {
        ProtectionDomain::allocate(self)
    }
}

impl Context {
    /// Opens a context for the given device and verifies port connectivity.
    ///
    /// This function performs the following steps:
    /// 1.  Calls `ibv_open_device` to establish a context.
    /// 2.  Verifies the RDMA port is in `ACTIVE` or `ARMED` state.
    ///
    /// # Errors
    ///
    /// *   Returns [`IbvError::Permission`] if the process lacks permission to access RDMA devices.
    /// *   Returns [`IbvError::Driver`] if `libibverbs` fails to open the device for OS-specific reasons.
    /// *   Returns [`IbvError::Resource`] if the RDMA port is `DOWN` or `INIT`, indicating the link is not ready.
    pub fn from_device(dev: &DeviceRef) -> IbvResult<Self> {
        // SAFETY: `dev.device_ptr` is guaranteed valid by the `DeviceRef` lifetime/invariants.
        let ibv_ctx = unsafe { ibv_open_device(dev.device_ptr) };
        if ibv_ctx.is_null() {
            return Err(IbvError::from_errno_with_msg(
                io::Error::last_os_error().raw_os_error().unwrap(),
                "Failed to open device context",
            ));
        }

        let context = Self {
            inner: Arc::new(ContextInner { ctx: ibv_ctx }),
        };

        // Enforce that the port is active/armed before returning a usable context.
        context.inner.query_port()?;

        log::debug!("Context opened");
        Ok(context)
    }
}

/// Inner wrapper to manage the lifecycle of the raw `ibv_context` pointer.
pub(crate) struct ContextInner {
    pub(crate) ctx: *mut ibv_context,
}

/// SAFETY: libibverbs components are thread safe.
unsafe impl Sync for ContextInner {}
/// SAFETY: libibverbs components are thread safe.
unsafe impl Send for ContextInner {}

impl Drop for ContextInner {
    fn drop(&mut self) {
        log::debug!("Context closed");
        // SAFETY: `self.ctx` is guaranteed valid and open.
        if unsafe { ibv_close_device(self.ctx) } != 0 {
            let error = IbvError::from_errno_with_msg(
                io::Error::last_os_error().raw_os_error().unwrap(),
                "Failed to close context",
            );
            log::error!("{error}");
        }
    }
}

impl std::fmt::Debug for ContextInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // SAFETY: The `DeviceRef` produced takes a pointer to a valid `ibv_device`
        // and is used for a shorter lifetime than self.
        f.debug_struct("Context")
            .field("device", &unsafe {
                DeviceRef::from_ptr((&*self.ctx).device)
            })
            .finish()
    }
}

impl ContextInner {
    /// Queries the properties of the primary port ([`IB_PORT`]).
    pub(crate) fn query_port(&self) -> IbvResult<ibv_port_attr> {
        let mut port_attr = ibv_port_attr::default();
        // SAFETY: `ibv_query_port` is a safe read operation if the context and pointer are valid.
        let errno = unsafe {
            ibv_query_port(
                self.ctx,
                IB_PORT,
                &mut port_attr as *mut ibv_port_attr as *mut _,
            )
        };
        if errno != 0 {
            return Err(IbvError::from_errno_with_msg(errno, "Failed to query port"));
        }

        match port_attr.state {
            ibv_port_state::IBV_PORT_ACTIVE | ibv_port_state::IBV_PORT_ARMED => Ok(port_attr),
            state => Err(IbvError::Resource(format!(
                "Port is in state {:?} (expected ACTIVE or ARMED)",
                state
            ))),
        }
    }
}
