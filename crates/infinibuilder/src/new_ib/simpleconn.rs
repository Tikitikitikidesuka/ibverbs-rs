use crate::new_ib::unsafe_slice::UnsafeSlice;
use dashmap::DashMap;
use derivative::Derivative;
use ibverbs::{
    CompletionQueue, MemoryRegion, PreparedQueuePair, ProtectionDomain, QueuePair,
    QueuePairEndpoint, RemoteMemoryRegion, ibv_qp_type, ibv_wc,
};
use std::mem::MaybeUninit;
use std::ops::RangeBounds;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct UnconnectedSimpleConnection<const CQ_SIZE: usize> {
    #[derivative(Debug = "ignore")]
    prepared_qp: PreparedQueuePair,
    qp_endpoint: QueuePairEndpoint,
    #[derivative(Debug = "ignore")]
    mr: MemoryRegion<UnsafeSlice<u8>>,
    #[derivative(Debug = "ignore")]
    pd: ProtectionDomain,
    #[derivative(Debug = "ignore")]
    cq: CompletionQueue,
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct SimpleConnection<const CQ_SIZE: usize> {
    self_qp_endpoint: QueuePairEndpoint,
    remote_qp_endpoint: QueuePairEndpoint,
    #[derivative(Debug = "ignore")]
    qp: QueuePair,
    #[derivative(Debug = "ignore")]
    mr: MemoryRegion<UnsafeSlice<u8>>,
    #[derivative(Debug = "ignore")]
    remote_mr: RemoteMemoryRegion,
    #[derivative(Debug = "ignore")]
    pd: ProtectionDomain,
    #[derivative(Debug = "ignore")]
    cached_cq: Arc<CachedCompletionQueue<CQ_SIZE>>,
    #[derivative(Debug = "ignore")]
    next_wr_id: AtomicU64,
}

impl<const CQ_SIZE: usize> UnconnectedSimpleConnection<CQ_SIZE> {
    pub fn connection_config(&self) -> SimpleConnectionEndpoint {
        SimpleConnectionEndpoint {
            qp_endpoint: self.qp_endpoint,
            remote_mr: self.mr.remote(),
        }
    }

    pub fn connect(
        self,
        connection_config: SimpleConnectionEndpoint,
    ) -> std::io::Result<SimpleConnection<CQ_SIZE>> {
        Ok(SimpleConnection {
            self_qp_endpoint: self.qp_endpoint,
            remote_qp_endpoint: connection_config.qp_endpoint,
            qp: self.prepared_qp.handshake(connection_config.qp_endpoint)?,
            mr: self.mr,
            remote_mr: connection_config.remote_mr,
            pd: self.pd,
            cached_cq: Arc::new(CachedCompletionQueue::new(self.cq)),
            next_wr_id: AtomicU64::new(0),
        })
    }
}

pub struct SimpleConnectionEndpoint {
    qp_endpoint: QueuePairEndpoint,
    remote_mr: RemoteMemoryRegion,
}

impl SimpleConnection<0> {
    /// SAFETY: Memory slice will have its ownership unlinked, meaning that it might be freed but this
    /// struct will still hold a reference to it which could result in illegal accesses to memory and UB.
    /// Memory is also taken as immutable reference, however by the nature of RDMA it is aliased and therefore
    /// can be mutated regardless.
    pub unsafe fn new<const CQ_SIZE: usize>(
        ib_context: &ibverbs::Context,
        memory: &[u8],
    ) -> std::io::Result<UnconnectedSimpleConnection<CQ_SIZE>> {
        let cq = ib_context.create_cq(CQ_SIZE as i32, 0)?;
        let pd = ib_context.alloc_pd()?;
        let mr = pd.register(unsafe { UnsafeSlice::new(memory) })?;
        let prepared_qp = pd.create_qp(&cq, &cq, ibv_qp_type::IBV_QPT_RC)?.build()?;
        let qp_endpoint = prepared_qp.endpoint()?;

        Ok(UnconnectedSimpleConnection {
            prepared_qp,
            qp_endpoint,
            mr,
            pd,
            cq,
        })
    }
}

impl<const CQ_SIZE: usize> SimpleConnection<CQ_SIZE> {
    /// # SAFETY
    /// The memory region can only be safely reused or dropped after the request is fully executed
    /// and a work completion has been retrieved from the corresponding completion queue
    /// (i.e., until CompletionQueue::poll returns a completion for this send).
    pub unsafe fn post_send(
        &mut self,
        mr_range: impl RangeBounds<usize>,
        imm_data: Option<u32>,
    ) -> std::io::Result<SimpleConnectionWorkRequest<CQ_SIZE>> {
        let wr_id = self.next_wr_id.fetch_add(1, Ordering::Relaxed);
        unsafe {
            self.qp
                .post_send(&[self.mr.slice(mr_range)], wr_id, imm_data)
        }?;
        Ok(SimpleConnectionWorkRequest::new(
            wr_id,
            self.cached_cq.clone(),
        ))
    }

    /// # SAFETY
    /// The memory region can only be safely reused or dropped after the request is fully executed
    /// and a work completion has been retrieved from the corresponding completion queue
    /// (i.e., until CompletionQueue::poll returns a completion for this receive)
    pub unsafe fn post_receive(
        &mut self,
        mr_range: impl RangeBounds<usize>,
    ) -> std::io::Result<SimpleConnectionWorkRequest<CQ_SIZE>> {
        let wr_id = self.next_wr_id.fetch_add(1, Ordering::Relaxed);
        unsafe { self.qp.post_receive(&[self.mr.slice(mr_range)], wr_id) }?;
        Ok(SimpleConnectionWorkRequest::new(
            wr_id,
            self.cached_cq.clone(),
        ))
    }

    pub fn post_write(
        &mut self,
        mr_range: impl RangeBounds<usize>,
        remote_mr_range: impl RangeBounds<usize>,
        imm_data: Option<u32>,
    ) -> std::io::Result<SimpleConnectionWorkRequest<CQ_SIZE>> {
        let wr_id = self.next_wr_id.fetch_add(1, Ordering::Relaxed);
        self.qp.post_write(
            &[self.mr.slice(mr_range)],
            self.remote_mr.slice(remote_mr_range),
            wr_id,
            imm_data,
        )?;
        Ok(SimpleConnectionWorkRequest::new(
            wr_id,
            self.cached_cq.clone(),
        ))
    }

    pub fn post_read(
        &mut self,
        mr_range: impl RangeBounds<usize>,
        remote_mr_slice: impl RangeBounds<usize>,
    ) -> std::io::Result<SimpleConnectionWorkRequest<CQ_SIZE>> {
        let wr_id = self.next_wr_id.fetch_add(1, Ordering::Relaxed);
        self.qp.post_read(
            &[self.mr.slice(mr_range)],
            self.remote_mr.slice(remote_mr_slice),
            wr_id,
        )?;
        Ok(SimpleConnectionWorkRequest::new(
            wr_id,
            self.cached_cq.clone(),
        ))
    }
}

struct CachedCompletionQueue<const CQ_SIZE: usize> {
    cq: Arc<CompletionQueue>,
    cq_cache: Arc<DashMap<u64, ibv_wc>>,
}

impl<const CQ_SIZE: usize> CachedCompletionQueue<CQ_SIZE> {
    pub fn new(cq: CompletionQueue) -> Self {
        Self {
            cq: Arc::new(cq),
            cq_cache: Arc::new(DashMap::new()),
        }
    }

    pub fn poll(&self) -> std::io::Result<usize> {
        let mut poll_buff: [ibv_wc; CQ_SIZE] = unsafe { MaybeUninit::uninit().assume_init() };
        let wc_slice = self.cq.poll(&mut poll_buff)?;

        // Fill cache with polled work completions
        for wc in wc_slice.iter() {
            self.cq_cache.insert(wc.wr_id(), *wc);
        }

        Ok(wc_slice.len())
    }

    pub fn consume(&self, wr_id: u64) -> Option<ibv_wc> {
        self.cq_cache.remove(&wr_id).map(|(_, wc)| wc)
    }
}

pub struct SimpleConnectionWorkRequest<const CQ_SIZE: usize> {
    wr_id: u64,
    cq_cache: Arc<CachedCompletionQueue<CQ_SIZE>>,
    opt_wc: Option<ibv_wc>,
}

impl<const CQ_SIZE: usize> SimpleConnectionWorkRequest<CQ_SIZE> {
    fn new(wr_id: u64, cq_cache: Arc<CachedCompletionQueue<CQ_SIZE>>) -> Self {
        Self {
            wr_id,
            cq_cache,
            opt_wc: None,
        }
    }

    pub fn poll(&mut self) -> std::io::Result<Option<ibv_wc>> {
        // Check if already polled completion
        if let Some(_) = &self.opt_wc {
            return Ok(self.opt_wc);
        }

        // Check if the wc is cached
        if let Some(_) = self._update_from_cache()? {
            return Ok(self.opt_wc);
        }

        // If not cached, poll the cq and check again
        if let Some(_) = self._update_from_cq()? {
            return Ok(self.opt_wc);
        }

        Ok(None)
    }

    pub fn wait(mut self) -> std::io::Result<ibv_wc> {
        // Poll all sources first
        self.poll()?;

        // If not in opt_wc or cache, it will come through cq
        loop {
            // Poll only the completion queue
            self._update_from_cq()?;
            if let Some(wc) = self.opt_wc {
                return Ok(wc);
            }
            std::hint::spin_loop();
        }
    }

    fn _update_from_cq(&mut self) -> Result<Option<ibv_wc>, std::io::Error> {
        // Poll the cq and check the cache
        self.cq_cache.poll()?;
        self._update_from_cache()
    }

    fn _update_from_cache(&mut self) -> Result<Option<ibv_wc>, std::io::Error> {
        // Check if the wc is cached
        match self.cq_cache.consume(self.wr_id) {
            Some(wc) => {
                self.opt_wc = Some(wc);
                Ok(Some(wc))
            }
            None => Ok(None),
        }
    }
}
