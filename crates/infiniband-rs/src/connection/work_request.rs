use crate::connection::cached_completion_queue::IbvCachedCompletionQueue;
use crate::connection::unsafe_member::UnsafeMember;
use crate::connection::work_completion::IbvWorkCompletion;
use crate::connection::work_error::IbvWorkError;
use std::cell::RefCell;
use std::fmt::{Debug, Formatter};
use std::io;
use std::marker::PhantomData;
use std::rc::Rc;

pub struct IbvWorkRequest<'a> {
    wr_id: u64,
    cq: Rc<RefCell<IbvCachedCompletionQueue>>,
    status: IbvWorkRequestStatus,

    /// SAFETY INVARIANT: The lifetime of the data must be the same as the lifetime of the work request.
    _data_lifetime: UnsafeMember<PhantomData<&'a [u8]>>,
}

impl<'a> IbvWorkRequest<'a> {
    pub(super) unsafe fn new(wr_id: u64, cq: Rc<RefCell<IbvCachedCompletionQueue>>) -> Self {
        Self {
            wr_id,
            cq,
            status: None,
            _data_lifetime: unsafe { UnsafeMember::new(PhantomData::<&'a [u8]>) },
        }
    }
}

impl<'a> Drop for IbvWorkRequest<'a> {
    fn drop(&mut self) {
        if let Err(e) = self.consume() {
            let debug_text = format!("{:?}", self);
            log::error!("({debug_text}) -> Failed to poll work request to completion: {e}")
        }
    }
}

impl<'a> Debug for IbvWorkRequest<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IbvWorkRequest")
            .field("wr_id", &self.wr_id)
            .field("status", &self.status)
            .finish()
    }
}

pub type IbvWorkResult = Result<IbvWorkCompletion, IbvWorkError>;
pub type IbvWorkRequestStatus = Option<IbvWorkResult>;

impl IbvWorkRequest<'_> {
    /// Returns `io::Error` if an error occurs while polling.
    /// If no error occurs, `Ok` contains:
    /// `None` if the work request is not yet complete.
    /// `IbvWorkRequestStatus` with the status of the work request.
    /// The work request status can be `Ok(WorkCompletion)` or `Err(io::Error)`
    /// if an error occurred during the work request's operation was run.
    pub fn poll(&mut self) -> io::Result<IbvWorkRequestStatus> {
        // Check if previously completed
        if let Some(result) = self.status {
            return Ok(Some(result));
        }

        // Check cache in case some other `IbvWorkRequest` polled the cq
        if let Some(status) = self.consume_cache() {
            self.status = Some(status);
            return Ok(Some(status));
        }

        // Otherwise, poll completion queue ourselves
        let polled_num = self.cq.borrow_mut().update()?;
        if polled_num > 0 {
            // Check cache again if we polled at least one wr
            if let Some(status) = self.consume_cache() {
                self.status = Some(status);
                return Ok(Some(status));
            }
        }

        Ok(None)
    }

    /// Polls the work request in a busy loop until it is complete.
    pub fn consume(&mut self) -> io::Result<IbvWorkResult> {
        loop {
            let status = self.poll()?;
            if let Some(wc) = status {
                return Ok(wc);
            }
        }
    }

    fn consume_cache(&mut self) -> IbvWorkRequestStatus {
        let mut cq = self.cq.borrow_mut();
        if let Some(wc) = cq.consume(self.wr_id) {
            if let Some((error_code, vendor_code)) = wc.error() {
                // Return work error if work failed
                Some(Err(IbvWorkError::new(error_code, vendor_code)))
            } else {
                // Return work completion if work succeeded
                // TODO: Fill in work completion
                Some(Ok(IbvWorkCompletion))
            }
        } else {
            // Work not completed yet
            None
        }
    }
}
