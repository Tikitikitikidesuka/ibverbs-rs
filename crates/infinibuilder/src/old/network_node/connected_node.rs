use crate::data_transmission::{IbBDataTransmissionConnection, UnsafeSlice};
use crate::{IbBConnectedEndpoint, IbBSyncBackend};
use ibverbs::{CompletionQueue, MemoryRegion, PreparedQueuePair, ProtectionDomain, QueuePair, QueuePairEndpoint};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io;
use std::rc::Rc;
use std::sync::Arc;

pub struct Connection {
    pub(crate) qp: QueuePair,
    pub(crate) cq: Arc<CompletionQueue>,
    pub(crate) pd: Arc<ProtectionDomain>,
    pub(crate) mr: Arc<MemoryRegion<UnsafeSlice>>,
    pub(crate) endpoint: QueuePairEndpoint,
}

pub struct IbBConnectedNode<
    DataTransmissionConnection: IbBDataTransmissionConnection,
    SyncConnection: IbBSyncBackend,
> {
    pub(crate) Vec<data_transmission_backend>: DataTransmissionConnection,
    pub(crate) sync_backend: SyncConnection,
    /*

    */
}

impl<DataTransmissionBackend: IbBDataTransmissionConnection, SyncBackend: IbBSyncBackend>
    IbBConnectedNode<DataTransmissionBackend, SyncBackend>
{
    pub fn endpoint(&self) -> QueuePairEndpoint {
        self.endpoint
    }

    pub fn connect(self, endpoint: QueuePairEndpoint) -> io::Result<IbBConnectedEndpoint> {
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
