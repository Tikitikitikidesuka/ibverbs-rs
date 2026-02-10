use crate::ibverbs::context::Context;
use crate::ibverbs::error::{IbvError, IbvResult};
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
    pub fn create(context: &Context, min_capacity: u32) -> IbvResult<CompletionQueue> {
        let min_cq_entries = min_capacity.try_into().map_err(|_| {
            IbvError::InvalidInput("Completion queue min_cq_entries must fit in an i32".to_string())
        })?;

        let cc = unsafe { ibv_create_comp_channel(context.inner.ctx) };
        if cc.is_null() {
            return Err(IbvError::from_errno_with_msg(
                io::Error::last_os_error().raw_os_error().unwrap(),
                "Failed to create completion channel",
            ));
        }

        // If this block returns Err, we catch it and destroy the channel before returning.
        let configure_channel = || -> IbvResult<()> {
            let cc_fd = unsafe { BorrowedFd::borrow_raw((*cc).fd) };

            let flags = nix::fcntl::fcntl(cc_fd, nix::fcntl::F_GETFL).map_err(|errno| {
                IbvError::from_errno_with_msg(
                    errno as i32,
                    "Failed to get completion channel flags",
                )
            })?;

            // The file descriptor needs to be set to non-blocking because `ibv_get_cq_event()`
            // would block otherwise.
            let arg = nix::fcntl::FcntlArg::F_SETFL(
                nix::fcntl::OFlag::from_bits_retain(flags) | nix::fcntl::OFlag::O_NONBLOCK,
            );

            nix::fcntl::fcntl(cc_fd, arg).map_err(|errno| {
                IbvError::from_errno_with_msg(
                    errno as i32,
                    "Failed to set completion channel to non-blocking",
                )
            })?;

            Ok(())
        };

        // Execute configuration and cleanup on failure
        if let Err(e) = configure_channel() {
            unsafe { ibv_destroy_comp_channel(cc) };
            return Err(e);
        }

        let cq = unsafe {
            ibv_create_cq(
                context.inner.ctx,
                min_cq_entries,
                ptr::null::<c_void>().offset(0) as *mut _,
                cc,
                0,
            )
        };

        if cq.is_null() {
            unsafe { ibv_destroy_comp_channel(cc) }; // Clean up channel on failure!

            return Err(IbvError::from_errno_with_msg(
                io::Error::last_os_error().raw_os_error().unwrap(),
                &format!("Failed to create completion queue with size {min_cq_entries}"),
            ));
        }

        log::debug!("CompletionQueue created");
        Ok(CompletionQueue {
            inner: Arc::new(CompletionQueueInner {
                context: context.clone(),
                cc,
                cq,
                min_capacity: min_cq_entries as u32,
            }),
        })
    }

    pub fn poll<'poll_buff>(
        &self,
        completions: &'poll_buff mut [PollSlot],
    ) -> IbvResult<PolledCompletions<'poll_buff>> {
        let ctx: *mut ibv_context = unsafe { &*self.inner.cq }.context;
        let ops = &mut unsafe { &mut *ctx }.ops;
        let num_polled = unsafe {
            ops.poll_cq.as_mut().unwrap()(
                self.inner.cq,
                completions.len() as i32,
                completions.as_mut_ptr() as *mut ibv_wc,
            )
        };

        if num_polled < 0 {
            Err(IbvError::from_errno_with_msg(
                num_polled.abs(),
                "Failed to poll completion queue",
            ))
        } else {
            Ok(PolledCompletions {
                wcs: &mut completions[0..num_polled as usize],
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
        log::debug!("CompletionQueue destroyed");

        let errno = unsafe { ibv_destroy_cq(self.cq) };
        if errno != 0 {
            let error = IbvError::from_errno_with_msg(errno, "Failed to destroy completion queue");
            log::error!("{error}");
        }

        let errno = unsafe { ibv_destroy_comp_channel(self.cc) };
        if errno != 0 {
            let error =
                IbvError::from_errno_with_msg(errno, "Failed to destroy completion channel");
            log::error!("{error}");
        }
    }
}

impl std::fmt::Debug for CompletionQueueInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompletionQueueInner")
            .field("handle", &(unsafe { *self.cq }).handle)
            .field("capacity", &(unsafe { *self.cq }).cqe)
            .field("context", &self.context)
            .finish()
    }
}
