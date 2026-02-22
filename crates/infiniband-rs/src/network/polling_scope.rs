use crate::channel::polling_scope::{PollingScope, ScopeError};
use crate::network::Node;

impl Node {
    pub fn scope<'env, F, T, E>(&'env mut self, f: F) -> Result<T, ScopeError<E>>
    where
        F: for<'scope> FnOnce(&mut PollingScope<'scope, 'env, Node>) -> Result<T, E>,
    {
        PollingScope::run(self, f)
    }
    pub fn manual_scope<'env, F, T, E>(&'env mut self, f: F) -> Result<T, E>
    where
        F: for<'scope> FnOnce(&mut PollingScope<'scope, 'env, Node>) -> Result<T, E>,
    {
        PollingScope::run_manual(self, f)
    }
}
