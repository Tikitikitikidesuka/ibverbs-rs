use crate::channel::polling_scope::PollingScope;
use crate::network::Node;
use crate::network::barrier::BarrierError;
use std::time::Duration;

impl<'scope, 'env> PollingScope<'scope, 'env, Node> {
    pub fn barrier(&mut self, peers: &[usize], timeout: Duration) -> Result<(), BarrierError> {
        self.inner.barrier(peers, timeout)
    }

    pub fn barrier_unchecked(
        &mut self,
        peers: &[usize],
        timeout: Duration,
    ) -> Result<(), BarrierError> {
        self.inner.barrier_unchecked(peers, timeout)
    }
}

impl Node {
    pub fn barrier(&mut self, peers: &[usize], timeout: Duration) -> Result<(), BarrierError> {
        self.barrier
            .barrier(&mut self.multi_channel, peers, timeout)
    }

    pub fn barrier_unchecked(
        &mut self,
        peers: &[usize],
        timeout: Duration,
    ) -> Result<(), BarrierError> {
        self.barrier
            .barrier_unchecked(&mut self.multi_channel, peers, timeout)
    }
}
