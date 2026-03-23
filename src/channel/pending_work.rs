use crate::channel::TransportResult;
use crate::channel::cached_completion_queue::CachedCompletionQueue;
use crate::ibverbs::work::{WorkResult, WorkSuccess};
use std::cell::RefCell;
use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;
use std::rc::Rc;

/// A handle to a posted work request that has not yet been polled to completion.
///
/// `PendingWork` ties the lifetime of the data buffers (`'a`) to the operation,
/// preventing the user from accessing or dropping the memory while the NIC may still
/// be performing DMA on it.
///
/// On drop, any unpolled work is automatically polled to completion via [`spin_poll`](Self::spin_poll).
/// This ensures the hardware is done with the buffers before they are released.
#[must_use = "PendingWork must be dropped to ensure completion"]
pub struct PendingWork<'a> {
    wr_id: u64,
    cq: Rc<RefCell<CachedCompletionQueue>>,
    status: Option<WorkResult>,

    /// SAFETY INVARIANT: The lifetime of the data must be the same as the lifetime of the work request.
    _data_lifetime: PhantomData<&'a [u8]>,
}

impl<'a> PendingWork<'a> {
    /// SAFETY INVARIANT: The lifetime of the data involved must be the same as the lifetime of the work request.
    pub(super) unsafe fn new(wr_id: u64, cq: Rc<RefCell<CachedCompletionQueue>>) -> Self {
        Self {
            wr_id,
            cq,
            status: None,
            _data_lifetime: PhantomData::<&'a [u8]>,
        }
    }
}

impl<'a> Drop for PendingWork<'a> {
    fn drop(&mut self) {
        if let Err(error) = self.spin_poll() {
            log::error!("Failed to poll pending work to completion: {error}")
        }
    }
}

impl<'a> Debug for PendingWork<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingWork")
            .field("wr_id", &self.wr_id)
            .field("status", &self.status)
            .finish()
    }
}

impl PendingWork<'_> {
    /// Returns the work request ID assigned to this operation.
    pub fn wr_id(&self) -> u64 {
        self.wr_id
    }

    /// Checks if the operation has completed.
    ///
    /// Returns `None` if the operation is still in progress, or `Some(result)` once complete.
    /// Subsequent calls after completion return the cached result.
    pub fn poll(&mut self) -> Option<TransportResult<WorkSuccess>> {
        // Check if previously completed
        if self.status.is_some() {
            return self.status.map(|res| res.map_err(Into::into));
        }

        let mut self_cq = self.cq.borrow_mut();

        // Check cache in case some other `IbvWorkRequest` polled the cq
        if let Some(status) = Self::consume_cache(self.wr_id, &mut self_cq) {
            self.status = Some(status);
            return self.status.map(|res| res.map_err(Into::into));
        }

        // Otherwise, poll completion queue ourselves
        let polled_num = match self_cq.update() {
            Err(e) => return Some(Err(e.into())),
            Ok(n) => n,
        };
        if polled_num > 0 {
            // Check cache again if we polled at least one wr
            if let Some(status) = Self::consume_cache(self.wr_id, &mut self_cq) {
                self.status = Some(status);
                return Some(status.map_err(Into::into));
            }
        }

        None
    }

    /// Busy-waits until the operation completes and returns the result.
    pub fn spin_poll(&mut self) -> TransportResult<WorkSuccess> {
        loop {
            match self.poll() {
                None => continue,              // not ready yet, spin
                Some(Ok(wc)) => return Ok(wc), // completed successfully
                Some(Err(e)) => return Err(e), // completed with transport error
            }
        }
    }

    fn consume_cache(wr_id: u64, cq: &mut CachedCompletionQueue) -> Option<WorkResult> {
        cq.consume(wr_id).map(|w| w.result())
    }
}
