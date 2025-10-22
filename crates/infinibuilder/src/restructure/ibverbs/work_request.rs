use crate::restructure::ibverbs::completion_queue::{CachedCompletionQueue, PollError};
use crate::restructure::ibverbs::work_completion::IbvWorkCompletion;
use crate::restructure::ibverbs::work_error::{IbvWorkError, IbvWorkErrorCode};
use crate::restructure::rdma_connection::{
    RdmaWorkRequest, RdmaWorkRequestStatus, WorkRequestPollError,
};
use ibverbs::ibv_wc_status;
use log::warn;
use std::cell::RefCell;
use std::convert::Infallible;
use std::rc::Rc;

#[derive(Debug)]
pub struct IbvWorkRequest {
    wr_id: u64,
    cq: Rc<RefCell<CachedCompletionQueue>>,
    status: RdmaWorkRequestStatus<IbvWorkCompletion, IbvWorkError>,
}

impl IbvWorkRequest {
    pub(super) fn new(wr_id: u64, cq: Rc<RefCell<CachedCompletionQueue>>) -> Self {
        Self {
            wr_id,
            cq,
            status: RdmaWorkRequestStatus::Pending,
        }
    }

    fn map_cached_status(
        status: &RdmaWorkRequestStatus<IbvWorkCompletion, IbvWorkError>,
    ) -> RdmaWorkRequestStatus<IbvWorkCompletion, WorkRequestPollError<std::io::Error, IbvWorkError>>
    {
        match status {
            RdmaWorkRequestStatus::Pending => RdmaWorkRequestStatus::Pending,
            RdmaWorkRequestStatus::Success(wc) => RdmaWorkRequestStatus::Success(wc.clone()),
            RdmaWorkRequestStatus::Error(err) => {
                RdmaWorkRequestStatus::Error(WorkRequestPollError::RdmaError(err.clone()))
            }
        }
    }
}

impl RdmaWorkRequest for IbvWorkRequest {
    type WC = IbvWorkCompletion;
    type RdmaError = IbvWorkError;
    type PollError = std::io::Error;

    fn poll(
        &mut self,
    ) -> RdmaWorkRequestStatus<IbvWorkCompletion, WorkRequestPollError<std::io::Error, IbvWorkError>>
    {
        const POLL_BUFFER_SIZE: usize = 32;

        // If completion already consumed from CQ, just return it
        if self.status.complete() {
            return Self::map_cached_status(&self.status);
        }

        // Otherwise poll from CQ
        let mut cq = self.cq.borrow_mut();
        let num_found = match cq.poll::<POLL_BUFFER_SIZE>() {
            Err(PollError::IoError(io_err)) => {
                return RdmaWorkRequestStatus::Error(WorkRequestPollError::PollError(io_err));
            }
            Err(PollError::CacheFull) => {
                warn!(
                    "Could not poll WR({}) due to completion cache overload",
                    self.wr_id
                );
                return RdmaWorkRequestStatus::Pending;
            }
            Ok(_) => {}
        };

        if let Some(wc) = cq.consume(self.wr_id) {
            let new_status = match wc.error() {
                None => RdmaWorkRequestStatus::Success(IbvWorkCompletion::new(wc)),
                Some((status, vendor_code)) => match IbvWorkErrorCode::try_from(status as u32) {
                    Ok(code) => RdmaWorkRequestStatus::Error(IbvWorkError::new(code, vendor_code)),
                    Err(()) => RdmaWorkRequestStatus::Success(IbvWorkCompletion::new(wc)),
                },
            };

            self.status = new_status.clone();
            Self::map_cached_status(&self.status)
        } else {
            RdmaWorkRequestStatus::Pending
        }
    }
}

impl Drop for IbvWorkRequest {
    fn drop(&mut self) {
        // If dropped, ensure the work request has been consumed from the CQ
        // (otherwise memory will leak due to unacknowledged completions)
        if matches!(self.poll(), RdmaWorkRequestStatus::Pending) {
            self.cq.borrow_mut().ignore(self.wr_id);
        }
    }
}
