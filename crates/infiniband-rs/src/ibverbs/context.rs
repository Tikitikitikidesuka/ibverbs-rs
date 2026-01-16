use crate::ibverbs::completion_queue::IbvCompletionQueue;
use crate::ibverbs::devices::IbvDevice;
use crate::ibverbs::protection_domain::IbvProtectionDomain;
use ibverbs_sys::*;
use std::io;
use std::sync::Arc;

/// The first port (port #1) of each HCA is an InfiniBand port
/// and the second port (port #2) is an Ethernet port.
pub(super) const IB_PORT: u8 = 1;

#[derive(Debug)]
pub struct IbvContext {
    inner: Arc<IbvContextInner>,
}

impl IbvContext {
    /// Create a completion queue (CQ).
    ///
    /// `min_cq_entries` defines the minimum size of the CQ. The actual created size can be equal
    /// or higher than this value. `id` is an opaque identifier that is echoed by
    /// `CompletionQueue::poll`.
    ///
    /// # Errors
    ///  - `EINVAL`: Invalid `min_cq_entries` (must be `1 <= cqe <= dev_cap.max_cqe`).
    ///  - `ENOMEM`: Not enough resources to create completion queue.
    // TODO: This should not be public... This library will expose a connection as an atomic unit
    pub fn create_cq(&self, min_cq_entries: u32, id: isize) -> io::Result<IbvCompletionQueue> {
        IbvCompletionQueue::create(self.inner.clone(), min_cq_entries, id)
    }

    /// Allocate a protection domain (PDs) for the device's context.
    // TODO: This should not be public... This library will expose a connection as an atomic unit
    pub fn allocate_pd(&self) -> io::Result<IbvProtectionDomain> {
        IbvProtectionDomain::allocate(self.inner.clone())
    }
}

impl IbvContext {
    /// Opens a context for the given device, and queries its port and gid.
    pub(super) fn with_device(dev: *mut ibv_device) -> io::Result<Self> {
        assert!(!dev.is_null());

        let ibv_ctx = unsafe { ibv_open_device(dev) };
        if ibv_ctx.is_null() {
            return Err(io::Error::other("failed to open device"));
        }

        let context = Self {
            inner: Arc::new(IbvContextInner { ctx: ibv_ctx }),
        };

        // Check that the port is active/armed.
        context.inner.query_port()?;

        log::debug!("IbvContext opened");
        Ok(context)
    }
}

pub(super) struct IbvContextInner {
    pub(super) ctx: *mut ibv_context,
}

unsafe impl Sync for IbvContextInner {}
unsafe impl Send for IbvContextInner {}

impl Drop for IbvContextInner {
    fn drop(&mut self) {
        log::debug!("IbvContext closed");
        let ctx = self.ctx;
        if unsafe { ibv_close_device(self.ctx) } != 0 {
            let debug_text = format!("{:?}", self);
            log::error!(
                "({debug_text}) -> Failed to close device with `ibv_close_device({ctx:p})`"
            );
        }
    }
}

impl std::fmt::Debug for IbvContextInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IbvContext")
            .field("device", &unsafe { IbvDevice::new((&*self.ctx).device) })
            .finish()
    }
}

impl IbvContextInner {
    /// Checks the port is ACTIVE or ARMED
    pub(super) fn query_port(&self) -> io::Result<ibv_port_attr> {
        let mut port_attr = ibv_port_attr::default();
        let errno = unsafe {
            ibv_query_port(
                self.ctx,
                IB_PORT,
                &mut port_attr as *mut ibv_port_attr as *mut _,
            )
        };
        if errno != 0 {
            return Err(io::Error::from_raw_os_error(errno));
        }

        match port_attr.state {
            ibv_port_state::IBV_PORT_ACTIVE | ibv_port_state::IBV_PORT_ARMED => {}
            _ => {
                return Err(io::Error::other("port is not ACTIVE or ARMED"));
            }
        }

        Ok(port_attr)
    }
}
