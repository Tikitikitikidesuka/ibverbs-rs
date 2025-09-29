use std::time::Duration;
use crate::rdma_traits::RdmaRendezvous;

#[derive(Debug, Copy, Clone)]
pub struct NoTimeoutRendezvousFn;

#[derive(Debug, Copy, Clone)]
pub struct TimeoutRendezvousFn {
    pub(super) timeout: Duration,
}

pub trait RendezvousFn {
    fn rendezvous<T: RdmaRendezvous>(&self, conn: &mut T) -> std::io::Result<()>;
    fn wait_for_peer_signal<T: RdmaRendezvous>(&self, conn: &mut T) -> std::io::Result<()>;
}

impl RendezvousFn for NoTimeoutRendezvousFn {
    fn rendezvous<T: RdmaRendezvous>(&self, conn: &mut T) -> std::io::Result<()> {
        conn.rendezvous()
    }

    fn wait_for_peer_signal<T: RdmaRendezvous>(&self, conn: &mut T) -> std::io::Result<()> {
        conn.wait_for_peer_signal()
    }
}

impl RendezvousFn for TimeoutRendezvousFn {
    fn rendezvous<T: RdmaRendezvous>(&self, conn: &mut T) -> std::io::Result<()> {
        conn.rendezvous_timeout(self.timeout)
    }

    fn wait_for_peer_signal<T: RdmaRendezvous>(&self, conn: &mut T) -> std::io::Result<()> {
        conn.wait_for_peer_signal_timeout(self.timeout)
    }
}