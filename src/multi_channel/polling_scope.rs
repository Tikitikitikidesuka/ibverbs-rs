use crate::channel::polling_scope::{PollingScope, ScopeError};
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::multi_channel::MultiChannel;

impl MultiChannel {
    pub fn scope<'env, F, T, E>(&'env mut self, f: F) -> Result<T, ScopeError<E>>
    where
        F: for<'scope> FnOnce(&mut PollingScope<'scope, 'env, MultiChannel>) -> Result<T, E>,
    {
        PollingScope::run(self, f)
    }

    pub fn manual_scope<'env, F, T, E>(&'env mut self, f: F) -> Result<T, E>
    where
        F: for<'scope> FnOnce(&mut PollingScope<'scope, 'env, MultiChannel>) -> Result<T, E>,
    {
        PollingScope::run_manual(self, f)
    }
}

impl<'scope, 'env> PollingScope<'scope, 'env, MultiChannel> {
    pub fn pd(&self) -> &ProtectionDomain {
        self.inner.pd()
    }

    pub fn num_channels(&self) -> usize {
        self.inner.channels.len()
    }
}
