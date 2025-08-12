use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use ibverbs::{ibv_wc, CompletionQueue, MemoryRegion, QueuePairEndpoint};
use crate::data_transmission::unsafe_slice::UnsafeSlice;

pub struct IbBDataTransmissionBackendImpl {
    pub(crate) data_mr: MemoryRegion<UnsafeSlice>,
    pub(crate) cq: Rc<CompletionQueue>,
    pub(crate) cq_size: usize,
    pub(crate) endpoint: QueuePairEndpoint,
    pub(crate) remote_endpoint: QueuePairEndpoint,
    pub(crate) next_wr_id: u64,
    pub(crate) wc_cache: Rc<RefCell<HashMap<u64, ibv_wc>>>,
    pub(crate) dead_wr: Rc<RefCell<HashSet<u64>>>,
}


