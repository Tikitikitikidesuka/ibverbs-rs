use crate::context::IbvContextInner;
use ibverbs_sys::*;
use std::ffi::c_void;
use std::os::fd::BorrowedFd;
use std::sync::Arc;
use std::{io, ptr};

#[derive(Debug)]
pub struct IbvCompletionQueue {
    pub(super) inner: Arc<IbvCompletionQueueInner>,
}

impl IbvCompletionQueue {
    /// Create a completion queue (CQ).
    ///
    /// `min_cq_entries` defines the minimum size of the CQ. The actual created size can be equal
    /// or higher than this value. `id` is an opaque identifier that is echoed by
    /// `CompletionQueue::poll`.
    ///
    /// # Errors
    ///  - `EINVAL`: Invalid `min_cq_entries` (must be `1 <= cqe <= dev_cap.max_cqe`).
    ///  - `ENOMEM`: Not enough resources to create completion queue.
    pub(super) fn create(
        context: Arc<IbvContextInner>,
        min_cq_entries: i32,
        id: isize,
    ) -> io::Result<Self> {
        let cc = unsafe { ibv_create_comp_channel(context.ctx) };
        if cc.is_null() {
            return Err(io::Error::last_os_error());
        }

        let cc_fd = unsafe { BorrowedFd::borrow_raw((*cc).fd) };
        let flags = nix::fcntl::fcntl(cc_fd, nix::fcntl::F_GETFL)?;
        // the file descriptor needs to be set to non-blocking because `ibv_get_cq_event()`
        // would block otherwise.
        let arg = nix::fcntl::FcntlArg::F_SETFL(
            nix::fcntl::OFlag::from_bits_retain(flags) | nix::fcntl::OFlag::O_NONBLOCK,
        );
        nix::fcntl::fcntl(cc_fd, arg)?;

        let cq = unsafe {
            ibv_create_cq(
                context.ctx,
                min_cq_entries,
                ptr::null::<c_void>().offset(id) as *mut _,
                cc,
                0,
            )
        };

        if cq.is_null() {
            let err = io::Error::last_os_error();
            let err = match err.kind() {
                io::ErrorKind::InvalidInput => io::Error::new(
                    err.kind(), // reuse
                    format!(
                        "invalid min_cq_entries ({min_cq_entries}) \
                        (must be 1 and dev_cap.max_cqe)"
                    ),
                ),
                io::ErrorKind::OutOfMemory => io::Error::new(
                    err.kind(), // reuse
                    "not enough resources to create completion queue",
                ),
                _ => err,
            };
            Err(err)
        } else {
            Ok(IbvCompletionQueue {
                inner: Arc::new(IbvCompletionQueueInner { context, cc, cq }),
            })
        }
    }
}

pub(super) struct IbvCompletionQueueInner {
    pub(super) context: Arc<IbvContextInner>,
    pub(super) cq: *mut ibv_cq,
    pub(super) cc: *mut ibv_comp_channel,
}

unsafe impl Send for IbvCompletionQueueInner {}
unsafe impl Sync for IbvCompletionQueueInner {}

impl Drop for IbvCompletionQueueInner {
    fn drop(&mut self) {
        let cq = self.cq;
        let debug_text = format!("{:?}", self);
        let errno = unsafe { ibv_destroy_cq(self.cq) };
        if errno != 0 {
            let e = io::Error::from_raw_os_error(errno);
            log::error!(
                "({debug_text}) -> Failed to release completion queue with `ibv_destroy_cq({cq:p})`: {e}"
            );
        }

        let errno = unsafe { ibv_destroy_comp_channel(self.cc) };
        if errno != 0 {
            let e = io::Error::from_raw_os_error(errno);
            log::error!(
                "({debug_text}) -> Failed to release completion channel with `ibv_destroy_cq({cq:p})`: {e}"
            );
        }
    }
}

impl std::fmt::Debug for IbvCompletionQueueInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IbvCompletionQueueInner")
            .field("handle", &(unsafe { *self.cq }).handle)
            .field("capacity", &(unsafe { *self.cq }).cqe)
            .field("context", &self.context)
            .finish()
    }
}
