use crate::restructure::ibverbs::completion_queue::{CachedCompletionQueue, PollError};
use crate::restructure::ibverbs::work_completion::IbvWorkCompletion;
use crate::restructure::ibverbs::work_error::{IbvWorkError, IbvWorkErrorCode};
use crate::restructure::rdma_connection::{RdmaWorkRequest, RdmaWorkRequestStatus};
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
}

impl RdmaWorkRequest for IbvWorkRequest {
    type WC = IbvWorkCompletion;
    type RdmaError = IbvWorkError;
    type PollError = std::io::Error;

    fn poll(
        &mut self,
    ) -> Result<RdmaWorkRequestStatus<Self::WC, Self::RdmaError>, Self::PollError> {
        const POLL_BUFFER_SIZE: usize = 32;

        // If completion already consumed from CQ, just return it
        if self.status.complete() {
            return Ok(self.status.clone());
        }

        // Otherwise poll from CQ
        let mut cq = self.cq.borrow_mut();
        if let Err(poll_error) = cq.poll::<POLL_BUFFER_SIZE>() {
            return match poll_error {
                PollError::IoError(io_error) => Err(io_error),
                PollError::CacheFull => {
                    warn!(
                        "Could not poll WR({}) due to completion cache overload",
                        self.wr_id
                    );
                    Ok(RdmaWorkRequestStatus::Pending)
                }
            };
        }

        // Check if found in CQ poll
        if let Some(wc) = cq.consume(self.wr_id) {
            let status = match wc.error() {
                None => RdmaWorkRequestStatus::Success(IbvWorkCompletion::new(wc)),
                Some((status, vendor_code)) => match IbvWorkErrorCode::try_from(status as u32) {
                    Ok(error_code) => {
                        RdmaWorkRequestStatus::Error(IbvWorkError::new(error_code, vendor_code))
                    }
                    Err(()) => RdmaWorkRequestStatus::Success(IbvWorkCompletion::new(wc)),
                },
            };
            self.status = status.clone();
            Ok(status)
        } else {
            Ok(RdmaWorkRequestStatus::Pending)
        }
    }
}

impl Drop for IbvWorkRequest {
    fn drop(&mut self) {
        // If dropped, it must be ensured that the work request is consumed
        // from the completion queue, otherwise memory will leak
        if let Ok(RdmaWorkRequestStatus::Pending) = self.poll() {
            // Just add it to the ignore list of the cached CQ
            self.cq.borrow_mut().ignore(self.wr_id)
        }
    }
}
