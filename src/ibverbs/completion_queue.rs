//! Completion Queue (CQ) management.
//!
//! A [`CompletionQueue`] (CQ) is the mechanism used to receive notifications about completed
//! Work Requests (WR) from a Queue Pair (QP). When a Send or Receive operation finishes,
//! the hardware writes a "Work Completion" (WC) entry into the CQ.
//!
//! # Polling
//!
//! This library uses direct polling for maximum performance. You must manually call
//! [`CompletionQueue::poll`] to check for completions. This operation bypasses the
//! kernel and reads directly from the hardware queue, ensuring minimal latency.
//!
//! # Example: Polling for Completions
//!
//! ```no_run
//! use ibverbs_rs::ibverbs;
//! use ibverbs_rs::ibverbs::completion_queue::PollSlot;
//!
//! let context = ibverbs::open_device("mlx5_0")?;
//! let cq = context.create_cq(16)?;
//!
//! // Pre-allocate a buffer for polling multiple completions at once
//! let mut slots = [PollSlot::default(); 16];
//!
//! // Poll for completions (non-blocking)
//! let completions = cq.poll(&mut slots)?;
//!
//! for wc in completions {
//!     println!("Work completion result: {:?}", wc.result());
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use crate::ibverbs::device::Context;
use crate::ibverbs::error::{IbvError, IbvResult};
use crate::ibverbs::work::WorkCompletion;
use ibverbs_sys::*;
use std::ffi::c_void;
use std::sync::Arc;
use std::{io, ptr};

/// A shared handle to a Completion Queue (CQ).
///
/// This struct is thread-safe ([`Sync`]) and reference-counted ([`Arc`]). It holds a strong reference
/// to the [`Context`] that created it, ensuring the device remains open.
#[derive(Debug, Clone)]
pub struct CompletionQueue {
    pub(super) inner: Arc<CompletionQueueInner>,
}

impl CompletionQueue {
    /// Creates a new Completion Queue.
    ///
    /// # Arguments
    ///
    /// * `context` — The device context on which to create the CQ.
    /// * `min_capacity` — The minimum number of completion entries this CQ must hold.
    ///   The hardware may allocate a larger queue.
    ///
    /// # Errors
    ///
    /// * Returns [`IbvError::InvalidInput`] if `min_capacity` is too large (exceeds device limits)
    ///   or cannot fit in an `i32`.
    /// * Returns [`IbvError::Resource`] if the system cannot allocate the necessary resources.
    pub fn create(context: &Context, min_capacity: u32) -> IbvResult<CompletionQueue> {
        let min_cq_entries = min_capacity.try_into().map_err(|_| {
            IbvError::InvalidInput("Completion queue min_cq_entries must fit in an i32".to_string())
        })?;

        let cq = unsafe {
            ibv_create_cq(
                context.inner.ctx,
                min_cq_entries,
                ptr::null::<c_void>().offset(0) as *mut _, // cq_context (user data), unused
                ptr::null::<ibv_comp_channel>() as *mut _, // comp_channel (NULL = polling only)
                0, // comp_vector (CPU affinity, unused w/o channel)
            )
        };

        if cq.is_null() {
            return Err(IbvError::from_errno_with_msg(
                io::Error::last_os_error().raw_os_error().unwrap_or(0),
                format!("Failed to create completion queue with size {min_cq_entries}"),
            ));
        }

        log::debug!("CompletionQueue created with capacity {}", min_capacity);
        Ok(CompletionQueue {
            inner: Arc::new(CompletionQueueInner {
                context: context.clone(),
                cq,
                min_capacity,
            }),
        })
    }

    /// Polls the CQ for completed work requests.
    ///
    /// This method checks the hardware queue for completions. It is non-blocking: if no completions
    /// are available, it returns an empty iterator immediately.
    ///
    /// # Arguments
    ///
    /// * `completions` — A mutable slice of [`PollSlot`]s. This buffer serves as the destination
    ///   where the NIC/driver will write the completion data. By requiring the caller to provide
    ///   this buffer, the library avoids internal heap allocations during the hot polling loop.
    ///   If the buffer length exceeds `i32::MAX`, only `i32::MAX` entries will be polled and
    ///   the remaining slots will be unused; a warning is logged in that case.
    ///
    /// # Returns
    ///
    /// Returns a [`PolledCompletions`] iterator wrapper. This iterator yields owned
    /// [`WorkCompletion`](crate::ibverbs::work::WorkCompletion) values constructed from the
    /// data copied by the NIC into the provided `completions` buffer.
    pub fn poll<'poll_buff>(
        &self,
        completions: &'poll_buff mut [PollSlot],
    ) -> IbvResult<PolledCompletions<'poll_buff>> {
        let ne = i32::try_from(completions.len()).unwrap_or_else(|_| {
            log::warn!(
                "poll buffer length {} exceeds i32::MAX; only {} entries will be polled",
                completions.len(),
                i32::MAX
            );
            i32::MAX
        });

        let ctx: *mut ibv_context = unsafe { &*self.inner.cq }.context;
        let ops = &mut unsafe { &mut *ctx }.ops;
        let num_polled = unsafe {
            ops.poll_cq.as_mut().unwrap()(
                self.inner.cq,
                ne,
                completions.as_mut_ptr() as *mut ibv_wc,
            )
        };

        if num_polled < 0 {
            Err(IbvError::from_errno_with_msg(
                num_polled.abs(),
                "Failed to poll completion queue",
            ))
        } else {
            // num_polled is non-negative after the check above
            #[allow(clippy::cast_sign_loss)]
            Ok(PolledCompletions {
                wcs: &mut completions[0..num_polled as usize],
            })
        }
    }

    /// Returns the minimum capacity of the Completion Queue.
    pub fn min_capacity(&self) -> u32 {
        self.inner.min_capacity
    }

    /// Returns a reference to the Context associated with this CQ.
    pub fn context(&self) -> &Context {
        &self.inner.context
    }
}

/// A pre-allocated slot for receiving a work completion.
///
/// This struct is a transparent wrapper around `ibv_wc`. Users should allocate an array
/// of these slots to pass to [`CompletionQueue::poll`].
#[derive(Copy, Clone, Debug, Default)]
#[repr(transparent)]
pub struct PollSlot {
    wc: ibv_wc,
}

/// An iterator over completions retrieved from a poll operation.
///
/// This struct is returned by [`CompletionQueue::poll`]. It borrows the underlying
/// [`PollSlot`] buffer and yields [`WorkCompletion`] objects.
pub struct PolledCompletions<'a> {
    wcs: &'a mut [PollSlot],
}

impl PolledCompletions<'_> {
    /// Returns the number of completions actually polled.
    pub fn len(&self) -> usize {
        self.wcs.len()
    }

    /// Returns true if no completions were polled.
    pub fn is_empty(&self) -> bool {
        self.wcs.is_empty()
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

/// Inner wrapper managing the raw CQ pointer.
pub(super) struct CompletionQueueInner {
    pub(super) context: Context,
    pub(super) cq: *mut ibv_cq,
    pub(super) min_capacity: u32,
}

/// SAFETY: libibverbs components are thread safe.
unsafe impl Send for CompletionQueueInner {}
/// SAFETY: libibverbs components are thread safe.
unsafe impl Sync for CompletionQueueInner {}

impl Drop for CompletionQueueInner {
    fn drop(&mut self) {
        log::debug!("CompletionQueue destroyed");

        // SAFETY: self.cq is valid for the lifetime of Inner.
        let errno = unsafe { ibv_destroy_cq(self.cq) };
        if errno != 0 {
            let error = IbvError::from_errno_with_msg(errno, "Failed to destroy completion queue");
            log::error!("{error}");
        }
    }
}

impl std::fmt::Debug for CompletionQueueInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // SAFETY: Dereferencing self.cq to read fields (handle, cqe) is safe
        // because the pointer is valid for the lifetime of Inner.
        f.debug_struct("CompletionQueueInner")
            .field("handle", &(unsafe { *self.cq }).handle)
            .field("capacity", &(unsafe { *self.cq }).cqe)
            .field("context", &self.context)
            .finish()
    }
}
