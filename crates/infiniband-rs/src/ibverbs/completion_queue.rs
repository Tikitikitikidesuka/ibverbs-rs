use crate::ibverbs::context::Context;
use crate::ibverbs::work_completion::WorkCompletion;
use ibverbs_sys::*;
use std::ffi::c_void;
use std::os::fd::BorrowedFd;
use std::sync::Arc;
use std::{io, ptr};

#[derive(Debug, Clone)]
pub struct CompletionQueue {
    pub(super) inner: Arc<CompletionQueueInner>,
}

impl CompletionQueue {
    /// Create a completion queue (CQ).
    ///
    /// `min_cq_entries` defines the minimum size of the CQ. The actual created size can be equal
    /// or higher than this value. `id` is an opaque identifier that is echoed by
    /// `CompletionQueue::poll`.
    ///
    /// # Errors
    ///  - `EINVAL`: Invalid `min_cq_entries` (must be `1 <= cqe <= dev_cap.max_cqe`).
    ///  - `ENOMEM`: Not enough resources to create completion queue.
    pub fn create(context: &Context, id: isize, min_capacity: u32) -> io::Result<Self> {
        let min_cq_entries = min_capacity.try_into().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "invalid min_cq_entries ({min_capacity}) \
                        (must be between 1 and dev_cap.max_cqe)"
                ),
            )
        })?;

        let cc = unsafe { ibv_create_comp_channel(context.inner.ctx) };
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
                context.inner.ctx,
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
            log::debug!("IbvCompletionQueue created");
            Ok(CompletionQueue {
                inner: Arc::new(CompletionQueueInner {
                    context: context.clone(),
                    cc,
                    cq,
                    min_capacity: min_cq_entries as u32,
                }),
            })
        }
    }

    pub fn poll<'poll_buff>(
        &self,
        completions: &'poll_buff mut [PollSlot],
    ) -> io::Result<PolledCompletions<'poll_buff>> {
        let ctx: *mut ibv_context = unsafe { &*self.inner.cq }.context;
        let ops = &mut unsafe { &mut *ctx }.ops;
        let n = unsafe {
            ops.poll_cq.as_mut().unwrap()(
                self.inner.cq,
                completions.len() as i32,
                completions.as_mut_ptr() as *mut ibv_wc,
            )
        };

        if n < 0 {
            Err(io::Error::other("ibv_poll_cq failed"))
        } else {
            Ok(PolledCompletions {
                wcs: &mut completions[0..n as usize],
            })
        }
    }

    pub fn min_capacity(&self) -> u32 {
        self.inner.min_capacity
    }
}

#[derive(Copy, Clone, Debug, Default)]
#[repr(transparent)]
pub struct PollSlot {
    wc: ibv_wc,
}

pub struct PolledCompletions<'a> {
    wcs: &'a mut [PollSlot],
}

impl PolledCompletions<'_> {
    pub fn len(&self) -> usize {
        self.wcs.len()
    }
}

impl<'a> IntoIterator for PolledCompletions<'a> {
    type Item = WorkCompletion;
    type IntoIter = std::iter::Map<std::slice::Iter<'a, PollSlot>, fn(&PollSlot) -> WorkCompletion>;

    fn into_iter(self) -> Self::IntoIter {
        self.wcs
            .iter()
            .map(|wc_slot| WorkCompletion::new(wc_slot.wc))
    }
}

pub(super) struct CompletionQueueInner {
    pub(super) context: Context,
    pub(super) cq: *mut ibv_cq,
    pub(super) cc: *mut ibv_comp_channel,
    pub(super) min_capacity: u32,
}

unsafe impl Send for CompletionQueueInner {}
unsafe impl Sync for CompletionQueueInner {}

impl Drop for CompletionQueueInner {
    fn drop(&mut self) {
        log::debug!("IbvCompletionQueue destroyed");

        let cq = self.cq;
        let errno = unsafe { ibv_destroy_cq(self.cq) };
        if errno != 0 {
            let debug_text = format!("{:?}", self);
            let e = io::Error::from_raw_os_error(errno);
            log::error!(
                "({debug_text}) -> Failed to release completion queue with `ibv_destroy_cq({cq:p})`: {e}"
            );
        }

        let cc = self.cc;
        let errno = unsafe { ibv_destroy_comp_channel(self.cc) };
        if errno != 0 {
            let debug_text = format!("{:?}", self);
            let e = io::Error::from_raw_os_error(errno);
            log::error!(
                "({debug_text}) -> Failed to release completion channel with `ibv_destroy_comp_channel({cc:p})`: {e}"
            );
        }
    }
}

impl std::fmt::Debug for CompletionQueueInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IbvCompletionQueueInner")
            .field("handle", &(unsafe { *self.cq }).handle)
            .field("capacity", &(unsafe { *self.cq }).cqe)
            .field("context", &self.context)
            .finish()
    }
}
