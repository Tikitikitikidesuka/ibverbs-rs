use ibverbs::ibv_qp_type::IBV_QPT_RC;
use ibverbs::{
    CompletionQueue, Context, MemoryRegion, PreparedQueuePair, ProtectionDomain, QueuePair,
    QueuePairEndpoint, ibv_wc,
};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ops::RangeBounds;
use std::rc::Rc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::Relaxed;

pub struct UnsetContext;
pub struct SetContext<'a>(&'a Context);

pub struct UnsetDataMemoryRegion;
pub struct SetDataMemoryRegion<'a>(&'a mut [u8]);

pub struct UnsetCompletionQueueSize;
pub struct SetCompletionQueueSize(usize);

// Attribute order is important since Rust drops attributes in order of declaration
// QP must be destroyed before CQ
pub struct IbBEndpointBuilder<ContextStatus, DataMemoryRegionStatus, CompletionQueueSizeStatus> {
    context: ContextStatus,
    data_mr: DataMemoryRegionStatus,
    cq_size: CompletionQueueSizeStatus,
}

impl IbBEndpointBuilder<UnsetContext, UnsetDataMemoryRegion, UnsetCompletionQueueSize> {
    pub fn new() -> Self {
        Self {
            context: UnsetContext,
            data_mr: UnsetDataMemoryRegion,
            cq_size: UnsetCompletionQueueSize,
        }
    }
}

impl<DataMemoryRegionStatus, CompletionQueueSizeStatus>
    IbBEndpointBuilder<UnsetContext, DataMemoryRegionStatus, CompletionQueueSizeStatus>
{
    pub fn set_context(
        self,
        context: &Context,
    ) -> IbBEndpointBuilder<SetContext, DataMemoryRegionStatus, CompletionQueueSizeStatus> {
        IbBEndpointBuilder {
            context: SetContext(context),
            data_mr: self.data_mr,
            cq_size: self.cq_size,
        }
    }
}

impl<ContextStatus, CompletionQueueSizeStatus>
    IbBEndpointBuilder<ContextStatus, UnsetDataMemoryRegion, CompletionQueueSizeStatus>
{
    pub fn set_data_memory_region(
        self,
        memory: &mut [u8],
    ) -> IbBEndpointBuilder<ContextStatus, SetDataMemoryRegion, CompletionQueueSizeStatus> {
        IbBEndpointBuilder {
            context: self.context,
            data_mr: SetDataMemoryRegion(memory),
            cq_size: self.cq_size,
        }
    }
}

impl<ContextStatus, DataMemoryRegionStatus>
    IbBEndpointBuilder<ContextStatus, DataMemoryRegionStatus, UnsetCompletionQueueSize>
{
    pub fn set_completion_queue_size(
        self,
        size: usize,
    ) -> IbBEndpointBuilder<ContextStatus, DataMemoryRegionStatus, SetCompletionQueueSize> {
        IbBEndpointBuilder {
            context: self.context,
            data_mr: self.data_mr,
            cq_size: SetCompletionQueueSize(size),
        }
    }
}

impl<'a> IbBEndpointBuilder<SetContext<'_>, SetDataMemoryRegion<'a>, SetCompletionQueueSize> {
    pub fn build(self) -> IbBUnconnectedEndpoint<'a> {
        let context = self.context.0;
        let cq_size = self.cq_size.0;
        let cq = context.create_cq(cq_size as i32, 0).unwrap();
        let pd = context.alloc_pd().unwrap();
        let prepared_qp = pd
            .create_qp(&cq, &cq, IBV_QPT_RC)
            .unwrap()
            //.set_gid_index(1)
            .build()
            .unwrap();
        let data_mr = pd.register(self.data_mr.0).unwrap();
        let endpoint = prepared_qp.endpoint().unwrap();

        IbBUnconnectedEndpoint {
            prepared_qp,
            cq,
            cq_size,
            pd,
            data_mr,
            endpoint,
        }
    }
}

pub struct IbBUnconnectedEndpoint<'a> {
    prepared_qp: PreparedQueuePair,
    cq: CompletionQueue,
    cq_size: usize,
    pd: ProtectionDomain,
    data_mr: MemoryRegion<&'a mut [u8]>,
    endpoint: QueuePairEndpoint,
}

impl<'a> IbBUnconnectedEndpoint<'a> {
    pub fn endpoint(&self) -> QueuePairEndpoint {
        self.endpoint
    }

    pub fn connect(self, endpoint: QueuePairEndpoint) -> IbBConnectedEndpoint<'a> {
        let qp = self.prepared_qp.handshake(endpoint).unwrap();

        IbBConnectedEndpoint {
            cq: Rc::new(self.cq),
            cq_size: self.cq_size,
            pd: self.pd,
            qp,
            data_mr: self.data_mr,
            endpoint: self.endpoint,
            remote_endpoint: endpoint,
            next_wr_id: AtomicU64::new(0),
            wc_cache: Rc::new(RefCell::new(HashMap::new())),
            dead_wr: Rc::new(RefCell::new(HashSet::new())),
        }
    }
}

// Attribute order is important since Rust drops attributes in order of declaration
// QP must be destroyed before CQ
pub struct IbBConnectedEndpoint<'a> {
    qp: QueuePair,
    pd: ProtectionDomain,
    data_mr: MemoryRegion<&'a mut [u8]>,
    cq: Rc<CompletionQueue>,
    cq_size: usize,
    endpoint: QueuePairEndpoint,
    remote_endpoint: QueuePairEndpoint,
    next_wr_id: AtomicU64,
    wc_cache: Rc<RefCell<HashMap<u64, ibv_wc>>>,
    dead_wr: Rc<RefCell<HashSet<u64>>>,
}

pub struct WorkRequest {
    id: u64,
    cq: Rc<CompletionQueue>,
    wc_cache: Rc<RefCell<HashMap<u64, ibv_wc>>>,
    dead_wr: Rc<RefCell<HashSet<u64>>>,
}

impl WorkRequest {
    fn gather_completions(&self) {
        const CQ_POLL_ARR_SIZE: usize = 16;
        let mut cq_poll_arr = [ibv_wc::default(); CQ_POLL_ARR_SIZE];

        // Get new completions
        let mut completions = self.cq.poll(&mut cq_poll_arr[..]).unwrap();
        while completions.len() != 0 {
            completions.into_iter().for_each(|completion| {
                // Insert it to the completion cache only if it is not a dead request
                if !self.dead_wr.borrow_mut().remove(&completion.wr_id()) {
                    self.wc_cache
                        .borrow_mut()
                        .insert(completion.wr_id(), *completion);
                }
            });
            completions = self.cq.poll(&mut cq_poll_arr[..]).unwrap();
        }
    }

    pub fn poll(&self) -> bool {
        self.gather_completions();
        self.wc_cache.borrow().contains_key(&self.id)
    }

    pub fn wait(self) {
        while !self.poll() {
            std::hint::spin_loop();
        }
    }
}

impl Drop for WorkRequest {
    fn drop(&mut self) {
        // If already completed, remove it
        if let None = self.wc_cache.borrow_mut().remove(&self.id) {
            println!("Request {:?} ignored... Forgetting it...", self.id);
            // If not completed, add it to the dead wr set
            self.dead_wr.borrow_mut().insert(self.id);
            // It must be removed later when inserted
        }
    }
}

impl IbBConnectedEndpoint<'_> {
    pub fn endpoint(&self) -> QueuePairEndpoint {
        self.endpoint
    }

    pub fn remote_endpoint(&self) -> QueuePairEndpoint {
        self.remote_endpoint
    }

    #[must_use = "Work request has to be polled or waited for"]
    pub fn post_send(&mut self, bounds: impl RangeBounds<usize>) -> WorkRequest {
        let wr_id = self.next_wr_id.fetch_add(1, Relaxed);
        unsafe { self.qp.post_send(&[self.data_mr.slice(bounds)], wr_id) }.unwrap();

        WorkRequest {
            id: wr_id,
            cq: self.cq.clone(),
            wc_cache: self.wc_cache.clone(),
            dead_wr: self.dead_wr.clone(),
        }
    }

    #[must_use = "Work request has to be polled or waited for"]
    pub fn post_receive(&mut self, bounds: impl RangeBounds<usize>) -> WorkRequest {
        let wr_id = self.next_wr_id.fetch_add(1, Relaxed);
        unsafe { self.qp.post_receive(&[self.data_mr.slice(bounds)], wr_id) }.unwrap();

        WorkRequest {
            id: wr_id,
            cq: self.cq.clone(),
            wc_cache: self.wc_cache.clone(),
            dead_wr: self.dead_wr.clone(),
        }
    }
}
