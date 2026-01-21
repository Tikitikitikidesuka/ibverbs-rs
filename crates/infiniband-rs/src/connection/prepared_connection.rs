use crate::connection::cached_completion_queue::CachedCompletionQueue;
use crate::connection::connection::Connection;
use crate::ibverbs::prepared_queue_pair::PreparedQueuePair;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::ibverbs::queue_pair_endpoint::QueuePairEndpoint;
use std::io;

#[derive(Debug)]
pub struct PreparedConnection {
    cq: CachedCompletionQueue,
    pd: ProtectionDomain,
    qp: PreparedQueuePair,
}

impl PreparedConnection {
    pub(super) fn new(
        cq: CachedCompletionQueue,
        pd: ProtectionDomain,
        qp: PreparedQueuePair,
    ) -> Self {
        Self { cq, pd, qp }
    }
}

impl PreparedConnection {
    pub fn endpoint(&self) -> QueuePairEndpoint {
        self.qp.endpoint()
    }

    pub fn handshake(self, endpoint: QueuePairEndpoint) -> io::Result<Connection> {
        let qp = self.qp.handshake(endpoint)?;
        Ok(Connection::new(self.cq, self.pd, qp))
    }
}
