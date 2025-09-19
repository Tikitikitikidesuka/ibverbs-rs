use crate::new_ib::cq_cache::CachedCompletionQueue;
use crate::new_ib::unsafe_slice::UnsafeSlice;
use crate::new_ib::{SendRecv, WorkRequest};
use ibverbs::{MemoryRegion, ProtectionDomain, QueuePair, ibv_qp_type, ibv_wc};
use std::ops::RangeBounds;
use std::sync::Arc;

pub struct IbVerbsMultiPeer {
    connections: Vec<Arc<IbVerbsConnection>>,
    mr: MemoryRegion<UnsafeSlice<u8>>,
    pd: ProtectionDomain,
}

pub struct IbVerbsPeer<const CQ_SIZE: usize> {
    connection: IbVerbsConnection,
    mr: MemoryRegion<UnsafeSlice<u8>>,
    pd: ProtectionDomain,
}

struct IbVerbsConnection<const CQ_SIZE: usize> {
    qp: QueuePair,
    cq_cache: Arc<CachedCompletionQueue<CQ_SIZE>>,
}

pub struct IbVerbsTransport<'a, const CQ_SIZE: usize> {
    qp: &'a mut QueuePair,
    mr: &'a MemoryRegion<UnsafeSlice<u8>>,
    cq_cache: Arc<CachedCompletionQueue<CQ_SIZE>>,
}

impl<const CQ_SIZE: usize> IbVerbsPeer<CQ_SIZE> {
    /// SAFETY: Converts the memory region into an UnsafeSlice, meaning its ownership is untied.
    /// The memory could be freed but the reference to it would remain in this struct.
    pub unsafe fn new(ib_context: ibverbs::Context, mr: &[u8]) -> std::io::Result<Self> {
        let cq = CachedCompletionQueue::new(&ib_context)?;
        let pd = ib_context.alloc_pd()?;
        let mr = pd.register(UnsafeSlice::new(mr))?;
        let prepared_qp = pd.create_qp(&cq, &cq, ibv_qp_type::IBV_QPT_RC)?.build();
        let qp_endpoints = prepared_qps
            .iter()
            .map(|pqp| pqp.endpoint())
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            cq,
            pd,
            mr,
            prepared_qps,
            qp_endpoints,
        })
    }
}

impl SendRecv for IbVerbsTransport<'_> {
    type Error = std::io::Error;

    fn post_send(
        &mut self,
        mr_range: impl RangeBounds<usize>,
    ) -> Result<impl WorkRequest, <Self as SendRecv>::Error> {
        let wr_id = self.cq_cache.reserve_wr_id();
        unsafe { self.qp.post_send(&[self.mr.slice(mr_range)], wr_id, None) }?;

        Ok(IbVerbsWorkRequest {
            wr_id,
            cq_cache: self.cq_cache.clone(),
        })
    }

    fn post_recv(
        &mut self,
        mr_range: impl RangeBounds<usize>,
    ) -> Result<impl WorkRequest, std::io::Error> {
        let wr_id = self.cq_cache.reserve_wr_id();
        unsafe { self.qp.post_receive(&[self.mr.slice(mr_range)], wr_id) }?;

        Ok(IbVerbsWorkRequest {
            wr_id,
            cq_cache: self.cq_cache.clone(),
        })
    }
}

pub struct IbVerbsWorkRequest {
    wr_id: u64,
    cq_cache: Arc<CachedCompletionQueue<64>>,
}

impl WorkRequest for IbVerbsWorkRequest {
    type WorkCompletion = ibv_wc;
    type WorkRequestError = std::io::Error;

    fn poll(&self) -> Result<Option<ibv_wc>, std::io::Error> {
        self.cq_cache.update_cache()?;
        Ok(self.cq_cache.consume_wc(self.wr_id))
    }
}
