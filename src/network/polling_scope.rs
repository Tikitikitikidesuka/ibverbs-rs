use crate::channel::polling_scope::{PollingScope, ScopeError};
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::network::Node;

impl Node {
    /// Opens a polling scope that automatically polls all outstanding work requests when it ends.
    ///
    /// See [`Channel::scope`](crate::channel::Channel::scope) for details on the scoping mechanism.
    pub fn scope<'env, F, T, E>(&'env mut self, f: F) -> Result<T, ScopeError<E>>
    where
        F: for<'scope> FnOnce(&mut PollingScope<'scope, 'env, Node>) -> Result<T, E>,
    {
        PollingScope::run(self, f)
    }
    /// Opens a polling scope that enforces manual polling of all work requests.
    ///
    /// See [`Channel::manual_scope`](crate::channel::Channel::manual_scope) for details.
    pub fn manual_scope<'env, F, T, E>(&'env mut self, f: F) -> Result<T, E>
    where
        F: for<'scope> FnOnce(&mut PollingScope<'scope, 'env, Node>) -> Result<T, E>,
    {
        PollingScope::run_manual(self, f)
    }
}

impl<'scope, 'env> PollingScope<'scope, 'env, Node> {
    /// Returns a reference to the shared [`ProtectionDomain`].
    pub fn pd(&self) -> &ProtectionDomain {
        self.inner.pd()
    }

    /// Returns the total number of nodes in the network.
    pub fn world_size(&self) -> usize {
        self.inner.world_size()
    }

    /// Returns this node's rank.
    pub fn rank(&self) -> usize {
        self.inner.rank()
    }
}
