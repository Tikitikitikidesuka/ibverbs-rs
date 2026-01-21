use crate::connection::cached_completion_queue::CachedCompletionQueue;
use crate::connection::unsafe_member::UnsafeMember;
use crate::ibverbs::work_completion::IbvWorkResult;
use crate::ibverbs::work_error::WorkError;
use crate::ibverbs::work_success::WorkSuccess;
use std::cell::RefCell;
use std::fmt::{Debug, Formatter};
use std::io;
use std::marker::PhantomData;
use std::rc::Rc;
use thiserror::Error;

#[must_use = "IbvWorkRequest must be dropped to ensure completion"]
pub struct WorkRequest<'a> {
    wr_id: u64,
    cq: Rc<RefCell<CachedCompletionQueue>>,
    status: Option<Result<WorkSuccess, WorkError>>,

    /// SAFETY INVARIANT: The lifetime of the data must be the same as the lifetime of the work request.
    _data_lifetime: UnsafeMember<PhantomData<&'a [u8]>>,
}

impl<'a> WorkRequest<'a> {
    pub(super) unsafe fn new(wr_id: u64, cq: Rc<RefCell<CachedCompletionQueue>>) -> Self {
        Self {
            wr_id,
            cq,
            status: None,
            _data_lifetime: unsafe { UnsafeMember::new(PhantomData::<&'a [u8]>) },
        }
    }
}

impl<'a> Drop for WorkRequest<'a> {
    fn drop(&mut self) {
        if !self.already_polled_to_completion() {
            log::warn!("IbvWorkRequest not manually polled to completion");
            if let Err(e) = self.spin_poll() {
                let debug_text = format!("{:?}", self);
                log::error!("({debug_text}) -> Failed to poll work request to completion: {e}")
            }
        }
    }
}

impl<'a> Debug for WorkRequest<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IbvWorkRequest")
            .field("wr_id", &self.wr_id)
            .field("status", &self.status)
            .finish()
    }
}

#[derive(Debug, Error)]
pub enum WorkPollError {
    #[error("Polling error: {0}")]
    PollError(#[from] io::Error),
    #[error("Polled work error: {0}")]
    WorkError(#[from] WorkError),
}

pub type WorkSpinPollResult = Result<WorkSuccess, WorkPollError>;
pub type WorkPollResult = Option<Result<WorkSuccess, WorkPollError>>;

pub type IbvWorkRequestStatus = Option<IbvWorkResult>;

impl WorkRequest<'_> {
    pub fn wr_id(&self) -> u64 {
        self.wr_id
    }

    /// Returns `io::Error` if an error occurs while polling.
    /// If no error occurs, `Ok` contains:
    /// `None` if the work request is not yet complete.
    /// `IbvWorkRequestStatus` with the status of the work request.
    /// The work request status can be `Ok(WorkCompletion)` or `Err(io::Error)`
    /// if an error occurred during the work request's operation was run.
    pub fn poll(&mut self) -> WorkPollResult {
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

    /// Polls the work request in a busy loop until it is complete.
    /// It consumes the work completion from the cache. This method
    /// must be used to guarantee a work completion is finished and
    /// to free space on the completion queue.
    pub fn spin_poll(&mut self) -> WorkSpinPollResult {
        loop {
            match self.poll() {
                None => continue,              // not ready yet, spin
                Some(Ok(wc)) => return Ok(wc), // completed successfully
                Some(Err(e)) => return Err(e), // work poll error
            }
        }
    }

    pub(super) fn already_polled_to_completion(&self) -> bool {
        self.status.is_some()
    }

    fn consume_cache(wr_id: u64, cq: &mut CachedCompletionQueue) -> IbvWorkRequestStatus {
        cq.consume(wr_id).map(|w| w.result())
    }
}
