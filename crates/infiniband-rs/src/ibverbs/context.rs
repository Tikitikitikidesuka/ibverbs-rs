use crate::ibverbs::completion_queue::CompletionQueue;
use crate::ibverbs::devices::DeviceRef;
use crate::ibverbs::error::{IbvError, IbvResult};
use crate::ibverbs::protection_domain::ProtectionDomain;
use ibverbs_sys::*;
use std::io;
use std::sync::Arc;

/// The first port (port #1) of each HCA is an InfiniBand port
/// and the second port (port #2) is an Ethernet port.
pub(super) const IB_PORT: u8 = 1;

#[derive(Debug, Clone)]
pub struct Context {
    pub(super) inner: Arc<ContextInner>,
}

impl Context {
    /// Create a completion queue (CQ).
    ///
    /// `min_cq_entries` defines the minimum size of the CQ. The actual created size can be equal
    /// or higher than this value. `id` is an opaque identifier that is echoed by
    /// `CompletionQueue::poll`.
    ///
    /// # Errors
    ///  - `EINVAL`: Invalid `min_cq_entries` (must be `1 <= cqe <= dev_cap.max_cqe`).
    ///  - `ENOMEM`: Not enough resources to create completion queue.
    pub fn create_cq(&self, id: isize, min_cq_entries: u32) -> IbvResult<CompletionQueue> {
        CompletionQueue::create(self, id, min_cq_entries)
    }

    /// Allocate a protection domain (PDs) for the device's context.
    pub fn allocate_pd(&self) -> IbvResult<ProtectionDomain> {
        ProtectionDomain::allocate(self)
    }
}

impl Context {
    /// Opens a context for the given device, and queries its port and gid.
    pub fn from_device(dev: &DeviceRef) -> IbvResult<Self> {
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

        // Check that the port is active/armed.
        context.inner.query_port()?;

        log::debug!("Context opened");
        Ok(context)
    }
}

pub(super) struct ContextInner {
    pub(super) ctx: *mut ibv_context,
}

unsafe impl Sync for ContextInner {}
unsafe impl Send for ContextInner {}

impl Drop for ContextInner {
    fn drop(&mut self) {
        log::debug!("Context closed");
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
        f.debug_struct("Context")
            .field("device", &unsafe {
                DeviceRef::from_ptr((&*self.ctx).device)
            })
            .finish()
    }
}

impl ContextInner {
    /// Checks the port is ACTIVE or ARMED
    pub(super) fn query_port(&self) -> IbvResult<ibv_port_attr> {
        let mut port_attr = ibv_port_attr::default();
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
