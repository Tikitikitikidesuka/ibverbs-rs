use crate::channel::polling_scope::{PollingScope, ScopeError};
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::multi_channel::MultiChannel;

impl MultiChannel {
    /// Opens a polling scope that automatically polls all outstanding work requests when it ends.
    ///
    /// See [`Channel::scope`](crate::channel::Channel::scope) for details on the scoping mechanism.
    pub fn scope<'env, F, T, E>(&'env mut self, f: F) -> Result<T, ScopeError<E>>
    where
        F: for<'scope> FnOnce(&mut PollingScope<'scope, 'env, MultiChannel>) -> Result<T, E>,
    {
        PollingScope::run(self, f)
    }

    /// Opens a polling scope that enforces manual polling of all work requests.
    ///
    /// See [`Channel::manual_scope`](crate::channel::Channel::manual_scope) for details.
    pub fn manual_scope<'env, F, T, E>(&'env mut self, f: F) -> Result<T, E>
    where
        F: for<'scope> FnOnce(&mut PollingScope<'scope, 'env, MultiChannel>) -> Result<T, E>,
    {
        PollingScope::run_manual(self, f)
    }
}

impl<'scope, 'env> PollingScope<'scope, 'env, MultiChannel> {
    /// Returns a reference to the shared [`ProtectionDomain`].
    pub fn pd(&self) -> &ProtectionDomain {
        self.inner.pd()
    }

    /// Returns the number of channels in the underlying [`MultiChannel`].
    pub fn num_channels(&self) -> usize {
        self.inner.channels.len()
    }
}
