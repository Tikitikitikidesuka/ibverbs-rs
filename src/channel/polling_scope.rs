use crate::channel::pending_work::PendingWork;
use crate::channel::{Channel, TransportError, TransportResult};
use crate::ibverbs::error::IbvResult;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::ibverbs::work::{
    ReadWorkRequest, ReceiveWorkRequest, SendWorkRequest, WorkSuccess, WriteWorkRequest,
};
use std::cell::RefCell;
use std::marker::PhantomData;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::rc::Rc;
use thiserror::Error;

impl Channel {
    /// Opens a polling scope that automatically polls all outstanding work requests when it ends.
    ///
    /// This is the primary safe way to perform RDMA operations. The closure receives a
    /// [`PollingScope`] through which operations can be posted. When the closure returns,
    /// any work requests that were not manually polled are automatically polled to completion
    /// before this method returns.
    ///
    /// See [`manual_scope`](Self::manual_scope) for a variant that enforces manual polling
    /// and returns the user's error type directly.
    ///
    /// # Error handling
    ///
    /// * If the closure returns `Err(E)`, the scope still auto-polls all outstanding work,
    ///   then returns `ScopeError::ClosureError(E)`.
    /// * If the closure returns `Ok(T)` but auto-polling encounters transport errors,
    ///   returns `ScopeError::AutoPollError(...)`.
    /// * If the closure panics, outstanding work is still polled for cleanup before
    ///   the panic is resumed.
    pub fn scope<'env, F, T, E>(&'env mut self, f: F) -> Result<T, ScopeError<E>>
    where
        F: for<'scope> FnOnce(&mut PollingScope<'scope, 'env, Channel>) -> Result<T, E>,
    {
        PollingScope::run(self, f)
    }

    /// Opens a polling scope that enforces manual polling of all work requests.
    ///
    /// Both [`scope`](Self::scope) and `manual_scope` allow the user to poll work manually.
    /// The difference is that `manual_scope` makes this the **contract**: it returns
    /// `Result<T, E>` directly instead of wrapping it in [`ScopeError`], avoiding
    /// unnecessary error handling. If the closure succeeds but leaves work unpolled,
    /// this method panics as a safety net.
    ///
    /// If the closure returns an error, outstanding work is still cleaned up without panicking.
    pub fn manual_scope<'env, F, T, E>(&'env mut self, f: F) -> Result<T, E>
    where
        F: for<'scope> FnOnce(&mut PollingScope<'scope, 'env, Channel>) -> Result<T, E>,
    {
        PollingScope::run_manual(self, f)
    }
}

impl<'scope, 'env> PollingScope<'scope, 'env, Channel> {
    /// Returns a reference to the [`ProtectionDomain`] of the underlying channel.
    pub fn pd(&self) -> &ProtectionDomain {
        self.inner.pd()
    }
}

/// Convenience alias for a [`Result`] with [`ScopeError`] as the default error type.
pub type ScopeResult<T> = Result<T, ScopeError>;

/// Error returned by [`Channel::scope`].
///
/// Distinguishes between errors originating from the user's closure and errors
/// discovered during automatic polling of outstanding work requests.
#[derive(Debug, Error)]
pub enum ScopeError<E = TransportError> {
    /// The user's closure returned an error.
    #[error("Closure error: {0}")]
    ClosureError(#[from] E),
    /// The closure succeeded, but one or more auto-polled work requests failed.
    #[error("Auto poll error: {0:?}")]
    AutoPollError(Vec<TransportError>),
}

impl<'a, 'b, C> PollingScope<'a, 'b, C> {
    /// Runs a closure inside an auto-polling scope, similar to [`std::thread::scope`].
    ///
    /// Work requests created inside the closure are tracked internally. When the closure
    /// returns, any that were not manually polled are automatically polled to completion.
    /// The user cannot leak work requests to escape the lifetime, because the handles are
    /// stored in a private structure owned by the scope.
    ///
    /// # Lifetimes
    ///
    /// * `'scope` — The lifetime of the scope itself. New operations may be posted and may
    ///   still be running during this period. It begins before the closure runs and ends
    ///   after all outstanding work has been polled, but before this method returns.
    /// * `'env` — The lifetime of data borrowed by the operations. Must outlive `'scope`,
    ///   meaning anything alive at the call site (e.g. local variables) can be borrowed.
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

    /// Runs a closure inside a strict polling scope.
    ///
    /// If the closure succeeds but any work requests were left unpolled, this method panics.
    /// If the closure fails, outstanding work is still cleaned up without panicking.
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

/// A scoped context for posting RDMA operations with automatic lifetime safety.
///
/// Created by [`Channel::scope`] or [`Channel::manual_scope`]. Operations posted through
/// a `PollingScope` return [`ScopedPendingWork`] handles that borrow the data buffers for
/// `'scope`, preventing aliasing while the hardware is accessing them.
///
/// When the scope ends, all unpolled work is automatically polled to completion, ensuring
/// that buffers are not released while the NIC may still be performing DMA.
///
/// This design mirrors [`std::thread::scope`] — the scope owns the work request handles
/// internally, so the user cannot leak them via [`std::mem::forget`].
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
    fn auto_poll(self) -> AutoPollResult {
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
        F: FnOnce(&mut C) -> IbvResult<&mut Channel>,
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
        F: FnOnce(&mut C) -> IbvResult<&mut Channel>,
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
        F: FnOnce(&mut C) -> IbvResult<&mut Channel>,
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
        F: FnOnce(&mut C) -> IbvResult<&mut Channel>,
    {
        let channel = channel_selector(self.inner)?;
        let wr = ScopedPendingWork::new(unsafe { channel.read_unpolled(wr)? });
        self.wrs.push(wr.clone());
        Ok(wr)
    }
}

/// A handle to a pending RDMA operation within a [`PollingScope`].
///
/// This handle can be used to manually poll for completion. If not polled by the time
/// the scope ends, the operation will be auto-polled.
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

    /// Checks if the operation has completed.
    ///
    /// Returns `None` if the operation is still in progress, or `Some(result)` once complete.
    pub fn poll(&self) -> Option<TransportResult<WorkSuccess>> {
        let mut wr = self.inner.borrow_mut();
        let poll = wr.wr.poll()?;
        wr.user_polled_to_completion = true;
        Some(poll)
    }

    /// Busy-waits until the operation completes and returns the result.
    pub fn spin_poll(&self) -> TransportResult<WorkSuccess> {
        let mut wr = self.inner.borrow_mut();
        let poll = wr.wr.spin_poll();
        wr.user_polled_to_completion = true;
        poll
    }
}
