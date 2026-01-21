use crate::connection::connection::Connection;
use crate::connection::work_request::{
    WorkPollError, WorkPollResult, WorkRequest, WorkSpinPollResult,
};
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use crate::ibverbs::work_error::WorkError;
use std::cell::RefCell;
use std::fmt::{Display, Formatter};
use std::io;
use std::marker::PhantomData;
use std::rc::Rc;
use thiserror::Error;

pub struct ConnectionScope<'scope, 'env: 'scope> {
    inner: &'env mut Connection,
    wrs: Vec<Rc<RefCell<WorkRequest<'scope>>>>,
    // for invariance of lifetimes, see `std::thread::scope`
    scope: PhantomData<&'scope mut &'scope ()>,
    env: PhantomData<&'env mut &'env ()>,
}

/// Error of a Connection Scope caught during clean up.
/// - PollError means there was an error polling the completion queue.
///   This means the completion queue and queue pair of the connection have
///   transitioned to the error state and therefore all of the work requests
///   were flushed uncompleted and with an error.
/// - WorkError means at least one work request failed during its execution.
///   This only specifies how many work requests failed. For more details do
///   not rely on automatic polling of the scoped connection.
#[derive(Debug, Error)]
pub enum ConnectionScopeError {
    PollError(#[from] io::Error),
    WorkError(Vec<WorkError>),
}

impl Display for ConnectionScopeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionScopeError::PollError(io_error) => {
                write!(
                    f,
                    "IbvConnectionScope poll error during clean-up: {io_error}"
                )
            }
            ConnectionScopeError::WorkError(work_errors) => {
                // Header line with count
                writeln!(
                    f,
                    "IbvConnectionScope {} work errors during clean-up:",
                    work_errors.len()
                )?;

                // Each work error on its own line with a bullet
                for err in work_errors {
                    writeln!(f, "- {}", err)?;
                }

                Ok(())
            }
        }
    }
}

impl<'scope, 'env> ConnectionScope<'scope, 'env> {
    // Important to notice. *Clean up does not fail*. The returned result represents the outcome
    // of the polled work requests during clean up. If it errors, it means some of the work
    // requests failed.
    pub(super) fn clean_up(self) -> Result<(), ConnectionScopeError> {
        let mut work_errors = Vec::new();
        for wr in &self.wrs {
            let mut wr = wr.borrow_mut();
            if !wr.already_polled_to_completion() {
                // Take care of errors to report them
                if let Err(error) = wr.spin_poll() {
                    match error {
                        WorkPollError::PollError(poll_error) => {
                            return Err(ConnectionScopeError::PollError(poll_error));
                        }
                        WorkPollError::WorkError(work_error) => work_errors.push(work_error),
                    }
                }
            }
        }

        if work_errors.is_empty() {
            Ok(())
        } else {
            Err(ConnectionScopeError::WorkError(work_errors))
        }
    }
}

impl<'scope, 'env> ConnectionScope<'scope, 'env> {
    pub(super) fn new(connection: &'env mut Connection) -> Self {
        ConnectionScope {
            inner: connection,
            wrs: vec![],
            scope: PhantomData,
            env: PhantomData,
        }
    }
}

impl<'scope, 'env> ConnectionScope<'scope, 'env> {
    // The slice cannot be used again until the work request is consumed,
    // so no overlapping operations can be done concurrently
    pub fn post_send(
        &mut self,
        sends: impl AsRef<[ScatterElement<'env>]>,
    ) -> io::Result<ScopedWorkRequest<'scope>> {
        let wr = Rc::new(RefCell::new(unsafe { self.inner.send_unpolled(sends)? }));
        self.wrs.push(wr.clone());
        Ok(ScopedWorkRequest {
            inner: wr,
            env: Default::default(),
        })
    }

    // The slice cannot be used again until the work request is consumed,
    // so no overlapping operations can be done concurrently
    pub fn post_receive(
        &mut self,
        receives: impl AsMut<[GatherElement<'env>]>,
    ) -> io::Result<ScopedWorkRequest<'scope>> {
        let wr = Rc::new(RefCell::new(unsafe {
            self.inner.receive_unpolled(receives)?
        }));
        self.wrs.push(wr.clone());
        Ok(ScopedWorkRequest {
            inner: wr,
            env: Default::default(),
        })
    }

    /*
    // Safety: The data at the remote memory region might be modified while the read is done.
    // It is the user's responsibility to ensure it is stable while the read is in progress.
    pub unsafe fn post_read(
        &'scope mut self,
        from_slice: &'env RemoteMrSlice,
        into_slice: &'env mut [u8],
    ) -> Result<IbvScopedWorkRequest<'scope, 'env>> {
        // TODO: Post to infiniband hardware

        let wr = WorkRequest {
            wr_id: 0, // Whatever id it is
            cq: self.cq.clone(),
            _data_lifetime: PhantomData,
        };

        self.wrs.push(wr.clone());

        Ok(wr.into())
    }

    // Safety: The data at the remote memory region will be modified regardless of its mutability
    // status. It is the user's responsibility to ensure no use of the memory is being done concurrently.
    pub unsafe fn post_write(
        &'scope mut self,
        from_slice: &'env [u8],
        into_slice: &'env RemoteMrSlice,
    ) -> Result<IbvScopedWorkRequest<'scope, 'env>> {
        // TODO: Post to infiniband hardware

        let wr = WorkRequest {
            wr_id: 0, // Whatever id it is
            cq: self.cq.clone(),
            _data_lifetime: PhantomData,
        };

        self.wrs.push(wr.clone());

        Ok(wr.into())
    }
    */
}

pub struct ScopedWorkRequest<'scope> {
    inner: Rc<RefCell<WorkRequest<'scope>>>,
    env: PhantomData<&'scope mut &'scope ()>,
}

impl<'scope> ScopedWorkRequest<'scope> {
    pub fn poll(&self) -> WorkPollResult {
        self.inner.borrow_mut().poll()
    }

    pub fn spin_poll(&self) -> WorkSpinPollResult {
        self.inner.borrow_mut().spin_poll()
    }
}
