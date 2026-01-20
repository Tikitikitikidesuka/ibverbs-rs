use crate::connection::cached_completion_queue::IbvCachedCompletionQueue;
use crate::connection::connection::IbvConnection;
use crate::ibverbs::memory_region::IbvMemoryRegion;
use crate::ibverbs::prepared_queue_pair::IbvPreparedQueuePair;
use crate::ibverbs::protection_domain::IbvProtectionDomain;
use crate::ibverbs::queue_pair_endpoint::IbvQueuePairEndpoint;
use std::collections::HashMap;
use std::io;

#[derive(Debug)]
pub struct IbvPreparedConnection {
    cq: IbvCachedCompletionQueue,
    pd: IbvProtectionDomain,
    qp: IbvPreparedQueuePair,
}

impl IbvPreparedConnection {
    pub(super) fn new(
        cq: IbvCachedCompletionQueue,
        pd: IbvProtectionDomain,
        qp: IbvPreparedQueuePair,
    ) -> Self {
        Self { cq, pd, qp }
    }
}

impl IbvPreparedConnection {
    pub fn endpoint(&self) -> IbvQueuePairEndpoint {
        self.qp.endpoint()
    }

    pub fn handshake(self, endpoint: IbvQueuePairEndpoint) -> io::Result<IbvConnection> {
        let qp = self.qp.handshake(endpoint)?;
        Ok(IbvConnection::new(self.cq, self.pd, qp))
    }
}
