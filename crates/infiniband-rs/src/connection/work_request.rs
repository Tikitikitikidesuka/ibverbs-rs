use crate::connection::cached_completion_queue::IbvCachedCompletionQueue;
use crate::connection::unsafe_member::UnsafeMember;
use crate::connection::work_completion::IbvWorkCompletion;
use crate::connection::work_error::IbvWorkError;
use ibverbs_sys::ibv_dereg_mr;
use std::cell::RefCell;
use std::io;
use std::marker::PhantomData;
use std::rc::Rc;

#[derive(Debug)]
pub struct IbvWorkRequest<'a> {
    wr_id: u64,
    cq: Rc<RefCell<IbvCachedCompletionQueue>>,
    status: IbvWorkRequestStatus,

    /// SAFETY INVARIANT: The lifetime of the data must be the same as the lifetime of the work request.
    _data_lifetime: UnsafeMember<PhantomData<&'a [u8]>>,
}

impl<'a> Drop for IbvWorkRequest<'a> {
    fn drop(&mut self) {
        if let Err(e) = self.spin_poll() {
            let debug_text = format!("{:?}", self);
            log::error!("({debug_text}) -> Failed to poll work request to completion: {e}")
        }
    }
}

type IbvWorkRequestStatus = Option<IbvWorkRequestResult>;
type IbvWorkRequestResult = Result<IbvWorkCompletion, IbvWorkError>;

impl IbvWorkRequest<'_> {
    /// Returns `io::Error` if an error occurs while polling.
    /// If no error occurs, `Ok` contains:
    /// `None` if the work request is not yet complete.
    /// `IbvWorkRequestStatus` with the status of the work request.
    /// The work request status can be `Ok(WorkCompletion)` or `Err(io::Error)`
    /// if an error occurred during the work request's operation was run.
    pub fn poll(&mut self) -> io::Result<IbvWorkRequestStatus> {
        // Check if previously polled
        if let Some(result) = self.status {
            return Ok(Some(result));
        }

        // Otherwise, poll completion queue
        let mut cq = self.cq.borrow_mut();
        let num_polled = cq.poll()?;
        // Return if nothing polled
        if num_polled == 0 {
            return Ok(None);
        }

        // Check if poll contained the wr
        if let Some(wc) = cq.consume(self.wr_id) {
            // Return work error if work failed
            if let Some((error_code, vendor_code)) = wc.error() {
                return Ok(Some(Err(IbvWorkError::new(error_code, vendor_code))));
            }

            // Store success for future polls
            // TODO: Fill in work completion
            self.status = Some(Ok(IbvWorkCompletion));
            // Return success
            return Ok(self.status);
        }

        Ok(None)
    }

    /// Polls the work request in a busy loop until it is complete.
    pub fn spin_poll(&mut self) -> io::Result<IbvWorkRequestStatus> {
        loop {
            let status = self.poll()?;
            if let Some(wc) = status {
                return Ok(Some(wc));
            }
        }
    }
}
