use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io;
use std::rc::Rc;
use ibverbs::{CompletionQueue, MemoryRegion, PreparedQueuePair, ProtectionDomain, QueuePairEndpoint};
use crate::IbBConnectedEndpoint;

pub struct IbBUnconnectedEndpoint<'a> {
    pub(crate) prepared_qp: PreparedQueuePair,
    pub(crate) cq: CompletionQueue,
    pub(crate) cq_size: usize,
    pub(crate) pd: ProtectionDomain,
    pub(crate) data_mr: MemoryRegion<&'a mut [u8]>,
    pub(crate) endpoint: QueuePairEndpoint,
}

impl<'a> IbBUnconnectedEndpoint<'a> {
    pub fn endpoint(&self) -> QueuePairEndpoint {
        self.endpoint
    }

    pub fn connect(self, endpoint: QueuePairEndpoint) -> io::Result<IbBConnectedEndpoint<'a>> {
        let qp = self.prepared_qp.handshake(endpoint)?;

        Ok(IbBConnectedEndpoint {
            cq: Rc::new(self.cq),
            cq_size: self.cq_size,
            pd: self.pd,
            qp,
            data_mr: self.data_mr,
            endpoint: self.endpoint,
            remote_endpoint: endpoint,
            next_wr_id: 0,
            wc_cache: Rc::new(RefCell::new(HashMap::new())),
            dead_wr: Rc::new(RefCell::new(HashSet::new())),
        })
    }
}
