use crate::channel::raw_channel::RawChannel;
use crate::channel::raw_channel::pending_work::{
    MultiWorkPollError, PendingWork, WorkPollError, WorkPollResult, WorkSpinPollResult,
};
use crate::ibverbs::remote_memory_region::{RemoteMemorySlice, RemoteMemorySliceMut};
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use crate::ibverbs::work_error::WorkError;
use crate::ibverbs::work_request::{
    ReadWorkRequest, ReceiveWorkRequest, SendWorkRequest, WriteWorkRequest,
};
use std::borrow::{Borrow, BorrowMut};
use std::cell::RefCell;
use std::fmt::{Display, Formatter};
use std::io;
use std::marker::PhantomData;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::rc::Rc;

impl<'a, 'b, C> PollingScope<'a, 'b, C> {
    /// This method allows to safely send and receive data in a subscope, similar to [`std::thread::scope`].
    ///
    /// Scoping solves the problem of users being able to access memory regions scheduled for
    /// an RDMA operation before it is complete. If the methods to send, receive, read, write, etc,
    /// were in this class, the returned work requests could be dropped before the operation finished.
    /// If the work requests implemented a Drop trait to poll before being dropped, the user could
    /// forget them beforehand safely anyway, and so access the memory before the operation finished.
    /// The solution for this, as proposed by Jonatan, is to use the same scoping method as the one used
    /// for scoped treads. In this way, the created work requests have a well defined lifetime —that of
    /// the scope— and are stored in a private structure such that the user cannot forget them to avoid polling.
    /// If they have not been polled at the end of the scope, they will be polled automatically.
    ///
    /// # Lifetimes
    ///
    /// Scoped rdma involves two lifetimes: `'scope` and `'env`.
    ///
    /// The `'scope` lifetime represents the lifetime of the scope itself.
    /// That is: the time during which new rdma operations may be issued,
    /// and also the time during which they might still be running.
    /// Once this lifetime ends, all operations are polled to completion.
    /// This lifetime starts within the `scope` function, before `f` (the argument to `scope`) starts.
    /// It ends after `f` returns and all scoped rdma operations have been completed, but before `scope` returns.
    ///
    /// The `'env` lifetime represents the lifetime of whatever is borrowed by the scoped threads.
    /// This lifetime must outlast the call to `scope`, and thus cannot be smaller than `'scope`.
    /// It can be as small as the call to `scope`, meaning that anything that outlives this call,
    /// such as local variables defined right before the scope, can be borrowed by the scope.
    ///
    /// The `'env: 'scope` bound is part of the definition of the `IbvConnectionScope` type.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use infiniband_rs::connection::connection::Connection;
    /// # let mut conn: Connection = unsafe { std::mem::zeroed() };
    /// let mut mem = [0u8; 1024];
    /// let mr = conn.register_mr("foo_mr", mem.as_mut_ptr(), mem.len()).unwrap();
    ///
    /// let (send_mem, recv_mem) = mem.split_at_mut(4);
    /// send_mem.copy_from_slice(&[1, 2, 3, 4]);
    /// conn.scope(|s| {
    ///     let wr0 = s.post_receive(&[mr.prepare_receive(recv_mem).unwrap()])
    ///     .unwrap();
    ///     let wr1 = s.post_send(&[mr.prepare_send(send_mem).unwrap()]).unwrap();
    ///     std::mem::forget(wr0);
    ///     std::mem::forget(wr1);
    /// });
    /// ```
    pub(crate) fn run<'env, F, R>(inner: &'env mut C, f: F) -> Result<R, MultiWorkPollError>
    where
        F: for<'scope> FnOnce(&mut PollingScope<'scope, 'env, C>) -> R,
    {
        let mut scope = PollingScope::new(inner);
        // The user's closure may panic after issuing work requests.
        // The panic has to be caught to ensure clean up for exception safety.
        let user_result = catch_unwind(AssertUnwindSafe(|| f(&mut scope)));
        let clean_up_result = scope.clean_up();
        match user_result {
            Ok(r) => clean_up_result.map(|_| r),
            Err(panic) => resume_unwind(panic),
        }
    }
}

pub struct PollingScope<'scope, 'env: 'scope, C> {
    pub(crate) inner: &'env mut C,
    wrs: Vec<Rc<RefCell<PendingWork<'scope>>>>,
    // for invariance of lifetimes, see `std::thread::scope`
    scope: PhantomData<&'scope mut &'scope ()>,
    env: PhantomData<&'env mut &'env ()>,
}

impl From<Vec<WorkError>> for MultiWorkPollError {
    fn from(errors: Vec<WorkError>) -> Self {
        MultiWorkPollError::WorkError(errors)
    }
}

impl Display for MultiWorkPollError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            MultiWorkPollError::PollError(io_error) => {
                write!(
                    f,
                    "IbvConnectionScope poll error during clean-up: {io_error}"
                )
            }
            MultiWorkPollError::WorkError(work_errors) => {
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

impl<'scope, 'env, C> PollingScope<'scope, 'env, C> {
    pub(super) fn new(inner: &'env mut C) -> Self {
        PollingScope {
            inner,
            wrs: vec![],
            scope: PhantomData,
            env: PhantomData,
        }
    }

    // Important to notice. *Clean up does not fail*. The returned result represents the outcome
    // of the polled work requests during clean up. If it errors, it means some of the work
    // requests failed.
    pub(super) fn clean_up(self) -> Result<(), MultiWorkPollError> {
        let mut work_errors = Vec::new();
        for wr in &self.wrs {
            let mut wr = RefCell::borrow_mut(wr);
            if !wr.already_polled_to_completion() {
                // Take care of errors to report them
                if let Err(error) = wr.spin_poll() {
                    match error {
                        WorkPollError::PollError(poll_error) => {
                            return Err(MultiWorkPollError::PollError(poll_error));
                        }
                        WorkPollError::WorkError(work_error) => work_errors.push(work_error),
                    }
                }
            }
        }

        if work_errors.is_empty() {
            Ok(())
        } else {
            Err(MultiWorkPollError::WorkError(work_errors))
        }
    }
}

impl<'scope, 'env, C> PollingScope<'scope, 'env, C> {
    // The slice cannot be used again until the work request is consumed,
    // so no overlapping operations can be done concurrently
    pub(crate) fn channel_post_send<F, E, WR>(
        &mut self,
        channel_selector: F,
        wr: WR,
    ) -> io::Result<ScopedPendingWork<'scope>>
    where
        F: FnOnce(&mut C) -> io::Result<&mut RawChannel>,
        E: AsRef<[GatherElement<'env>]>,
        WR: Borrow<SendWorkRequest<'env, E>>,
    {
        let channel = channel_selector(self.inner)?;
        let wr = Rc::new(RefCell::new(unsafe { channel.send_unpolled(wr)? }));
        self.wrs.push(wr.clone());
        Ok(ScopedPendingWork {
            inner: wr,
            env: Default::default(),
        })
    }

    // The slice cannot be used again until the work request is consumed,
    // so no overlapping operations can be done concurrently
    pub(crate) fn channel_post_receive<F, E, WR>(
        &mut self,
        channel_selector: F,
        wr: WR,
    ) -> io::Result<ScopedPendingWork<'scope>>
    where
        F: FnOnce(&mut C) -> io::Result<&mut RawChannel>,
        E: AsMut<[ScatterElement<'env>]>,
        WR: BorrowMut<ReceiveWorkRequest<'env, E>>,
    {
        let channel = channel_selector(self.inner)?;
        let wr = Rc::new(RefCell::new(unsafe { channel.receive_unpolled(wr)? }));
        self.wrs.push(wr.clone());
        Ok(ScopedPendingWork {
            inner: wr,
            env: Default::default(),
        })
    }

    pub(crate) fn channel_post_write<F, E, R, WR>(
        &mut self,
        channel_selector: F,
        wr: WR,
    ) -> io::Result<ScopedPendingWork<'scope>>
    where
        F: FnOnce(&mut C) -> io::Result<&mut RawChannel>,
        E: AsRef<[GatherElement<'env>]>,
        R: BorrowMut<RemoteMemorySliceMut<'env>>,
        WR: BorrowMut<WriteWorkRequest<'env, E, R>>,
    {
        let channel = channel_selector(self.inner)?;
        let wr = Rc::new(RefCell::new(unsafe { channel.write_unpolled(wr)? }));
        self.wrs.push(wr.clone());
        Ok(ScopedPendingWork {
            inner: wr,
            env: Default::default(),
        })
    }

    pub(crate) fn channel_post_read<F, E, R, WR>(
        &mut self,
        channel_selector: F,
        wr: WR,
    ) -> io::Result<ScopedPendingWork<'scope>>
    where
        F: FnOnce(&mut C) -> io::Result<&mut RawChannel>,
        E: AsMut<[ScatterElement<'env>]>,
        R: Borrow<RemoteMemorySlice<'env>>,
        WR: BorrowMut<ReadWorkRequest<'env, E, R>>,
    {
        let channel = channel_selector(self.inner)?;
        let wr = Rc::new(RefCell::new(unsafe { channel.read_unpolled(wr)? }));
        self.wrs.push(wr.clone());
        Ok(ScopedPendingWork {
            inner: wr,
            env: Default::default(),
        })
    }
}

pub struct ScopedPendingWork<'scope> {
    inner: Rc<RefCell<PendingWork<'scope>>>,
    env: PhantomData<&'scope mut &'scope ()>,
}

impl<'scope> ScopedPendingWork<'scope> {
    pub fn poll(&self) -> WorkPollResult {
        RefCell::borrow_mut(&self.inner).poll()
    }
    pub fn spin_poll(&self) -> WorkSpinPollResult {
        RefCell::borrow_mut(&self.inner).spin_poll()
    }
}
