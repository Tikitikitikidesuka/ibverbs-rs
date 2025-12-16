use crate::connection::connection::IbvConnection;
use crate::ibverbs::completion_queue::IbvCompletionQueue;
use crate::ibverbs::memory_region::IbvMemoryRegion;
use crate::ibverbs::prepared_queue_pair::IbvPreparedQueuePair;
use crate::ibverbs::protection_domain::IbvProtectionDomain;
use crate::ibverbs::queue_pair_endpoint::IbvQueuePairEndpoint;
use std::collections::HashMap;
use std::io;

#[derive(Debug)]
pub struct IbvPreparedConnection {
    pub(super) cq: IbvCompletionQueue,
    pub(super) pd: IbvProtectionDomain,
    pub(super) qp: IbvPreparedQueuePair,
    pub(super) mrs: HashMap<String, IbvMemoryRegion>,
}

impl IbvPreparedConnection {
    pub fn endpoint(&self) -> IbvQueuePairEndpoint {
        self.qp.endpoint()
    }

    pub fn handshake(self, endpoint: IbvQueuePairEndpoint) -> io::Result<IbvConnection> {
        let qp = self.qp.handshake(endpoint)?;
        Ok(IbvConnection {
            cq: self.cq,
            pd: self.pd,
            qp,
            mrs: self.mrs,
        })
    }
}
