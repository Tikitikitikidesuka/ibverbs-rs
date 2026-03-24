use crate::channel::PollingScope;
use crate::network::Node;
use crate::network::barrier::BarrierError;
use std::time::Duration;

impl<'scope, 'env> PollingScope<'scope, 'env, Node> {
    /// Synchronizes with the given peers, blocking until all have reached the barrier or timeout.
    pub fn barrier(&mut self, peers: &[usize], timeout: Duration) -> Result<(), BarrierError> {
        self.inner.barrier(peers, timeout)
    }

    /// Like [`barrier`](Self::barrier), but skips validation of the peer list.
    pub fn barrier_unchecked(
        &mut self,
        peers: &[usize],
        timeout: Duration,
    ) -> Result<(), BarrierError> {
        self.inner.barrier_unchecked(peers, timeout)
    }
}

impl Node {
    /// Synchronizes with the given peers, blocking until all have reached the barrier or timeout.
    pub fn barrier(&mut self, peers: &[usize], timeout: Duration) -> Result<(), BarrierError> {
        self.barrier
            .barrier(&mut self.multi_channel, peers, timeout)
    }

    /// Like [`barrier`](Self::barrier), but skips validation of the peer list.
    pub fn barrier_unchecked(
        &mut self,
        peers: &[usize],
        timeout: Duration,
    ) -> Result<(), BarrierError> {
        self.barrier
            .barrier_unchecked(&mut self.multi_channel, peers, timeout)
    }
}
