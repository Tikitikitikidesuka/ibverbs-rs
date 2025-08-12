use crate::infiniband::unsafe_slice::UnsafeSlice;
use crate::{IbBSyncBackend, WorkRequest};
use ibverbs::{
    CompletionQueue, MemoryRegion, ProtectionDomain, QueuePair, QueuePairEndpoint, ibv_wc,
};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io;
use std::ops::RangeBounds;
use std::rc::Rc;

// Attribute order is important since Rust drops attributes in order of declaration
// QP must be destroyed before CQ
pub struct IbBConnectedEndpoint<SyncBackend: IbBSyncBackend> {
    pub(crate) qp: QueuePair,
    pub(crate) pd: ProtectionDomain,
    pub(crate) sync_backend: SyncBackend,
    pub(crate) data_tranmission_bakcned: IbBDataTransmissionBackend,
}


impl<SyncBackend: IbBSyncBackend> IbBConnectedEndpoint<SyncBackend> {
    pub fn endpoint(&self) -> QueuePairEndpoint {
        self.endpoint
    }

    pub fn remote_endpoint(&self) -> QueuePairEndpoint {
        self.remote_endpoint
    }

    pub fn post_send(&mut self, bounds: impl RangeBounds<usize>) -> io::Result<WorkRequest> {
        let wr_id = self.next_wr_id;
        self.next_wr_id += 1;

        unsafe { self.qp.post_send(&[self.data_mr.slice(bounds)], wr_id) }?;

        Ok(WorkRequest {
            id: wr_id,
            cq: self.cq.clone(),
            wc_cache: self.wc_cache.clone(),
            dead_wr: self.dead_wr.clone(),
        })
    }

    pub fn post_receive(&mut self, bounds: impl RangeBounds<usize>) -> io::Result<WorkRequest> {
        let wr_id = self.next_wr_id;
        self.next_wr_id += 1;

        unsafe { self.qp.post_receive(&[self.data_mr.slice(bounds)], wr_id) }?;

        Ok(WorkRequest {
            id: wr_id,
            cq: self.cq.clone(),
            wc_cache: self.wc_cache.clone(),
            dead_wr: self.dead_wr.clone(),
        })
    }
}
