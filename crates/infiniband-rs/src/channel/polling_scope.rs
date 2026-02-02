use crate::channel::pending_work::PendingWork;
use crate::channel::{Channel, TransportError, TransportResult};
use crate::ibverbs::error::{IbvError, IbvResult};
use crate::ibverbs::work_error::WorkError;
use crate::ibverbs::work_request::*;
use crate::ibverbs::work_success::WorkSuccess;
use std::cell::RefCell;
use std::io;
use std::marker::PhantomData;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::rc::Rc;
use thiserror::Error;

/// T is user closure Ok output type
/// E is user closure Err output type
/// If user closure returns Err(E) -> auto poll -> return E
/// If user closure returns Ok(T)
///     If auto poll returns io err -> return io err
///     If auto poll returns work err -> return work err
///     If auto poll returns Ok -> return user closure's Ok(T)
pub type ScopeResult<T, E> = Result<T, ScopeError<E>>;

#[derive(Debug, Error)]
pub enum ScopeError<E> {
    ClosureError(E),
    AutoPollError(Vec<TransportError>),
}

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
    pub(crate) fn run<'env, F, T, E>(inner: &'env mut C, f: F) -> Result<T, ScopeError<E>>
    where
        F: for<'scope> FnOnce(&mut PollingScope<'scope, 'env, C>) -> Result<T, E>,
    {
        let mut scope = PollingScope::new(inner);
        // The user's closure may panic after issuing work requests.
        // The panic has to be caught to ensure clean up for exception safety.
        let scope_result = catch_unwind(AssertUnwindSafe(|| f(&mut scope)));
        let auto_poll_result = scope.auto_poll();

        match scope_result {
            Ok(closure_result) => match closure_result {
                Err(closure_error) => Err(ScopeError::ClosureError(closure_error)),
                Ok(closure_output) => match auto_poll_result {
                    Ok(_) => Ok(closure_output),
                    Err(error) => Err(ScopeError::AutoPollError(error)),
                },
            },
            Err(panic) => resume_unwind(panic),
        }
    }

    /// Still safe by cleaning up. But if the closure succeeds (returns Ok(...)) but the autopoll
    /// hast to poll manually any wr, it panics.
    /// If the closure fails, however, the autopoll will be done but not panic and the error of the
    /// closure will be returned.
    pub(crate) fn run_manual<'env, F, T, E>(inner: &'env mut C, f: F) -> Result<T, E>
    where
        F: for<'scope> FnOnce(&mut PollingScope<'scope, 'env, C>) -> Result<T, E>,
    {
        let mut scope = PollingScope::new(inner);
        // The user's closure may panic after issuing work requests.
        // The panic has to be caught to ensure clean up for exception safety.
        let scope_result = catch_unwind(AssertUnwindSafe(|| f(&mut scope)));
        let auto_poll_result = scope.auto_poll();

        match scope_result {
            Ok(closure_result) => {
                let closure_output = closure_result?;
                match auto_poll_result {
                    Ok(AutoPollSuccess::NoPendingWorks) => Ok(closure_output),
                    Ok(AutoPollSuccess::PendingWorksSucceeded) | Err(_) => {
                        panic!("Unpolled wrs in PollingScope::run_manual")
                    }
                }
            }
            Err(panic) => resume_unwind(panic),
        }
    }
}

pub struct PollingScope<'scope, 'env: 'scope, C> {
    pub(crate) inner: &'env mut C,
    wrs: Vec<ScopedPendingWork<'scope>>,
    // for invariance of lifetimes, see `std::thread::scope`
    scope: PhantomData<&'scope mut &'scope ()>,
    env: PhantomData<&'env mut &'env ()>,
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

    // Important to notice. *Auto-poll does not fail*. The returned result represents the outcome
    // of the polled work requests during clean up. If it errors, it means some of the work
    // requests failed.
    // Auto polls all non manually polled work requests issued during the closure.
    pub(super) fn auto_poll(self) -> AutoPollResult {
        let mut auto_polled = false;
        let mut transport_errors = Vec::new();

        for wr in self.wrs {
            let mut wr = wr.inner.borrow_mut();
            // Only raise error into the auto polled if not polled by the user
            if !wr.user_polled_to_completion {
                auto_polled = true; // Mark that user left some wrs unpolled
                if let Err(transport_error) = wr.wr.spin_poll() {
                    transport_errors.push(transport_error);
                }
            }
        }

        // If everything goes well, no heap allocation
        if !auto_polled {
            Ok(AutoPollSuccess::NoPendingWorks)
        } else {
            if transport_errors.is_empty() {
                Ok(AutoPollSuccess::PendingWorksSucceeded)
            } else {
                Err(transport_errors)
            }
        }
    }
}

type AutoPollResult = Result<AutoPollSuccess, Vec<TransportError>>;

enum AutoPollSuccess {
    NoPendingWorks,
    PendingWorksSucceeded,
}

impl<'scope, 'env, C> PollingScope<'scope, 'env, C> {
    // The slice cannot be used again until the work request is consumed,
    // so no overlapping operations can be done concurrently
    pub(crate) fn channel_post_send<F>(
        &mut self,
        channel_selector: F,
        wr: SendWorkRequest<'_, 'env>,
    ) -> IbvResult<ScopedPendingWork<'scope>>
    where
        F: FnOnce(&mut C) -> io::Result<&mut Channel>,
    {
        let channel = channel_selector(self.inner)?;
        let wr = ScopedPendingWork::new(unsafe { channel.send_unpolled(wr)? });
        self.wrs.push(wr.clone());
        Ok(wr)
    }

    // The slice cannot be used again until the work request is consumed,
    // so no overlapping operations can be done concurrently
    pub(crate) fn channel_post_receive<F>(
        &mut self,
        channel_selector: F,
        wr: ReceiveWorkRequest<'_, 'env>,
    ) -> IbvResult<ScopedPendingWork<'scope>>
    where
        F: FnOnce(&mut C) -> io::Result<&mut Channel>,
    {
        let channel = channel_selector(self.inner)?;
        let wr = ScopedPendingWork::new(unsafe { channel.receive_unpolled(wr)? });
        self.wrs.push(wr.clone());
        Ok(wr)
    }

    pub(crate) fn channel_post_write<F>(
        &mut self,
        channel_selector: F,
        wr: WriteWorkRequest<'_, 'env>,
    ) -> IbvResult<ScopedPendingWork<'scope>>
    where
        F: FnOnce(&mut C) -> io::Result<&mut Channel>,
    {
        let channel = channel_selector(self.inner)?;
        let wr = ScopedPendingWork::new(unsafe { channel.write_unpolled(wr)? });
        self.wrs.push(wr.clone());
        Ok(wr)
    }

    pub(crate) fn channel_post_read<F>(
        &mut self,
        channel_selector: F,
        wr: ReadWorkRequest<'_, 'env>,
    ) -> IbvResult<ScopedPendingWork<'scope>>
    where
        F: FnOnce(&mut C) -> io::Result<&mut Channel>,
    {
        let channel = channel_selector(self.inner)?;
        let wr = ScopedPendingWork::new(unsafe { channel.read_unpolled(wr)? });
        self.wrs.push(wr.clone());
        Ok(wr)
    }
}

#[derive(Debug, Clone)]
pub struct ScopedPendingWork<'scope> {
    inner: Rc<RefCell<ScopedPendingWorkInner<'scope>>>,
    env: PhantomData<&'scope mut &'scope ()>,
}

#[derive(Debug)]
struct ScopedPendingWorkInner<'scope> {
    user_polled_to_completion: bool,
    wr: PendingWork<'scope>,
}

impl<'scope> ScopedPendingWork<'scope> {
    fn new(wr: PendingWork<'scope>) -> Self {
        ScopedPendingWork {
            inner: Rc::new(RefCell::new(ScopedPendingWorkInner {
                user_polled_to_completion: false,
                wr,
            })),
            env: PhantomData,
        }
    }

    pub fn poll(&self) -> Option<TransportResult<WorkSuccess>> {
        let mut wr = self.inner.borrow_mut();
        let poll = wr.wr.poll()?;
        wr.user_polled_to_completion = true;
        Some(poll)
    }

    pub fn spin_poll(&self) -> TransportResult<WorkSuccess> {
        let mut wr = self.inner.borrow_mut();
        let poll = wr.wr.spin_poll();
        wr.user_polled_to_completion = true;
        poll
    }
}
