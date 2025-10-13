use crate::restructure::ibverbs::completion_queue::{CachedCompletionQueue, PollError};
use crate::restructure::ibverbs::work_completion::IbvWorkCompletion;
use crate::restructure::rdma_connection::{RdmaWorkRequest, RdmaWorkRequestStatus};
use std::cell::RefCell;
use std::rc::Rc;
use log::warn;

#[derive(Debug)]
pub struct IbvWorkRequest {
    wr_id: u64,
    cq: Rc<RefCell<CachedCompletionQueue>>,
    wc: Option<IbvWorkCompletion>,
}

impl IbvWorkRequest {
    pub(super) fn new(wr_id: u64, cq: Rc<RefCell<CachedCompletionQueue>>) -> Self {
        Self { wr_id, cq, wc: None }
    }
}

impl RdmaWorkRequest for IbvWorkRequest {
    type WC = IbvWorkCompletion;
    type PollError = std::io::Error;

    fn poll(&mut self) -> RdmaWorkRequestStatus<Self::WC, Self::PollError> {
        const POLL_BUFFER_SIZE: usize = 32;

        // If completion already consumed from CQ, just return it
        if let Some(wc) = self.wc {
            return RdmaWorkRequestStatus::Success(wc);
        }

        // Otherwise poll from CQ
        let mut cq = self.cq.borrow_mut();
        if let Err(poll_error) = cq.poll::<POLL_BUFFER_SIZE>() {
            match poll_error {
                PollError::IoError(io_error) => return RdmaWorkRequestStatus::Error(io_error),
                PollError::CacheFull => {
                    warn!("Could not poll WR({}) due to completion cache overload", self.wr_id);
                    return RdmaWorkRequestStatus::Pending;
                },
            }
        }

        // Check if found in CQ poll
        if let Some(wc) = cq.consume(self.wr_id) {
            let wc = IbvWorkCompletion::new(wc);
            self.wc = Some(wc);
            RdmaWorkRequestStatus::Success(wc)
        } else {
            RdmaWorkRequestStatus::Pending
        }
    }
}

impl Drop for IbvWorkRequest {
    fn drop(&mut self) {
        // If dropped, it must be ensured that the work request is consumed
        // from the completion queue, otherwise memory will leak
        if let RdmaWorkRequestStatus::Pending = self.poll() {
            // Just add it to the ignore list of the cached CQ
            self.cq.borrow_mut().ignore(self.wr_id)
        }
    }
}