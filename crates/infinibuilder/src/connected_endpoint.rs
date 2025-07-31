use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io;
use std::ops::RangeBounds;
use std::rc::Rc;
use ibverbs::{ibv_wc, CompletionQueue, MemoryRegion, ProtectionDomain, QueuePair, QueuePairEndpoint};
use crate::WorkRequest;

// Attribute order is important since Rust drops attributes in order of declaration
// QP must be destroyed before CQ
pub struct IbBConnectedEndpoint<'a> {
    pub(crate) qp: QueuePair,
    pub(crate) pd: ProtectionDomain,
    pub(crate) data_mr: MemoryRegion<&'a mut [u8]>,
    pub(crate) cq: Rc<CompletionQueue>,
    pub(crate) cq_size: usize,
    pub(crate) endpoint: QueuePairEndpoint,
    pub(crate) remote_endpoint: QueuePairEndpoint,
    pub(crate) next_wr_id: u64,
    pub(crate) wc_cache: Rc<RefCell<HashMap<u64, ibv_wc>>>,
    pub(crate) dead_wr: Rc<RefCell<HashSet<u64>>>,
}

impl IbBConnectedEndpoint<'_> {
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
