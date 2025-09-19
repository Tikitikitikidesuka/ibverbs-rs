use crate::ibverbs::cached_cq::CachedCompletionQueue;
use crate::ibverbs::work_request::CachedWorkRequest;
use crate::rdma_traits::{RdmaReadWrite, RdmaRendezvous, RdmaSendRecv, WorkRequest};
use crate::unsafe_slice::UnsafeSlice;
use derivative::Derivative;
use ibverbs::{
    CompletionQueue, MemoryRegion, PreparedQueuePair, ProtectionDomain, QueuePair,
    QueuePairEndpoint, RemoteMemoryRegion, ibv_access_flags, ibv_qp_type, ibv_wc, ibv_wc_status,
};
use std::ops::{Deref, Range, RangeBounds};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct UnconnectedIBSimpleUnit<const CQ_SIZE: usize> {
    #[derivative(Debug = "ignore")]
    prepared_qp: PreparedQueuePair,
    qp_endpoint: QueuePairEndpoint,
    #[derivative(Debug = "ignore")]
    mr: MemoryRegion<UnsafeSlice<u8>>,
    #[derivative(Debug = "ignore")]
    rendezvous_state: Box<RendezvousMemoryRegion>,
    #[derivative(Debug = "ignore")]
    rendezvous_mr: MemoryRegion<UnsafeSlice<RendezvousState>>,
    #[derivative(Debug = "ignore")]
    pd: ProtectionDomain,
    #[derivative(Debug = "ignore")]
    cq: CompletionQueue,
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct IBSimpleUnit<const CQ_SIZE: usize> {
    self_qp_endpoint: QueuePairEndpoint,
    remote_qp_endpoint: QueuePairEndpoint,
    #[derivative(Debug = "ignore")]
    qp: QueuePair,
    #[derivative(Debug = "ignore")]
    mr: MemoryRegion<UnsafeSlice<u8>>,
    #[derivative(Debug = "ignore")]
    remote_mr: RemoteMemoryRegion,
    #[derivative(Debug = "ignore")]
    rendezvous_state: Box<RendezvousMemoryRegion>,
    #[derivative(Debug = "ignore")]
    rendezvous_mr: MemoryRegion<UnsafeSlice<RendezvousState>>,
    #[derivative(Debug = "ignore")]
    remote_rendezvous_mr: RemoteMemoryRegion,
    #[derivative(Debug = "ignore")]
    pd: ProtectionDomain,
    #[derivative(Debug = "ignore")]
    cached_cq: Arc<CachedCompletionQueue<CQ_SIZE>>,
    #[derivative(Debug = "ignore")]
    next_wr_id: AtomicU64,
}

impl<const CQ_SIZE: usize> UnconnectedIBSimpleUnit<CQ_SIZE> {
    pub fn connection_config(&self) -> SimpleConnectionEndpoint {
        SimpleConnectionEndpoint {
            qp_endpoint: self.qp_endpoint,
            remote_mr: self.mr.remote(),
            remote_rendezvous_mr: self.rendezvous_mr.remote(),
        }
    }

    pub fn connect(
        self,
        connection_config: SimpleConnectionEndpoint,
    ) -> std::io::Result<IBSimpleUnit<CQ_SIZE>> {
        Ok(IBSimpleUnit {
            self_qp_endpoint: self.qp_endpoint,
            remote_qp_endpoint: connection_config.qp_endpoint,
            qp: self.prepared_qp.handshake(connection_config.qp_endpoint)?,
            mr: self.mr,
            remote_mr: connection_config.remote_mr,
            rendezvous_state: self.rendezvous_state,
            rendezvous_mr: self.rendezvous_mr,
            remote_rendezvous_mr: connection_config.remote_rendezvous_mr,
            pd: self.pd,
            cached_cq: Arc::new(CachedCompletionQueue::new(self.cq)),
            next_wr_id: AtomicU64::new(0),
        })
    }
}

pub struct SimpleConnectionEndpoint {
    qp_endpoint: QueuePairEndpoint,
    remote_mr: RemoteMemoryRegion,
    remote_rendezvous_mr: RemoteMemoryRegion,
}

impl IBSimpleUnit<0> {
    /// SAFETY: Memory slice will have its ownership unlinked, meaning that it might be freed but this
    /// struct will still hold a reference to it which could result in illegal accesses to memory and UB.
    /// Memory is also taken as immutable reference, however by the nature of RDMA it is aliased and therefore
    /// can be mutated regardless.
    pub unsafe fn new<const CQ_SIZE: usize>(
        ib_context: &ibverbs::Context,
        memory: &[u8],
    ) -> std::io::Result<UnconnectedIBSimpleUnit<CQ_SIZE>> {
        let cq = ib_context.create_cq(CQ_SIZE as i32, 0)?;
        let pd = ib_context.alloc_pd()?;
        let mr = pd.register(unsafe { UnsafeSlice::new(memory) })?;
        let rendezvous_state = Box::new(RendezvousMemoryRegion::new());
        let rendezvous_mr = pd.register(unsafe { UnsafeSlice::new(rendezvous_state.as_ref()) })?;
        let prepared_qp = pd
            .create_qp(&cq, &cq, ibv_qp_type::IBV_QPT_RC)?
            .set_access(
                ibv_access_flags::IBV_ACCESS_REMOTE_WRITE
                    | ibv_access_flags::IBV_ACCESS_REMOTE_READ
                    | ibv_access_flags::IBV_ACCESS_LOCAL_WRITE,
            )
            .build()?;
        let qp_endpoint = prepared_qp.endpoint()?;

        Ok(UnconnectedIBSimpleUnit {
            prepared_qp,
            qp_endpoint,
            mr,
            rendezvous_state,
            rendezvous_mr,
            pd,
            cq,
        })
    }
}

impl<const CQ_SIZE: usize> RdmaSendRecv for IBSimpleUnit<CQ_SIZE> {
    /// # SAFETY
    /// The memory region can only be safely reused or dropped after the request is fully executed
    /// and a work completion has been retrieved from the corresponding completion queue
    /// (i.e., until CompletionQueue::poll returns a completion for this send).
    unsafe fn post_send(
        &mut self,
        mr_range: impl RangeBounds<usize>,
        imm_data: Option<u32>,
    ) -> Result<impl WorkRequest, std::io::Error> {
        let wr_id = self.next_wr_id.fetch_add(1, Ordering::Relaxed);
        unsafe {
            self.qp
                .post_send(&[self.mr.slice(mr_range)], wr_id, imm_data)
        }?;
        Ok(CachedWorkRequest::new(wr_id, self.cached_cq.clone()))
    }

    /// # SAFETY
    /// The memory region can only be safely reused or dropped after the request is fully executed
    /// and a work completion has been retrieved from the corresponding completion queue
    /// (i.e., until CompletionQueue::poll returns a completion for this receive)
    unsafe fn post_receive(
        &mut self,
        mr_range: impl RangeBounds<usize>,
    ) -> Result<impl WorkRequest, std::io::Error> {
        let wr_id = self.next_wr_id.fetch_add(1, Ordering::Relaxed);
        unsafe { self.qp.post_receive(&[self.mr.slice(mr_range)], wr_id) }?;
        Ok(CachedWorkRequest::new(wr_id, self.cached_cq.clone()))
    }
}

impl<const CQ_SIZE: usize> RdmaReadWrite for IBSimpleUnit<CQ_SIZE> {
    /// TODO: WRITE SAFETY
    unsafe fn post_write(
        &mut self,
        mr_range: impl RangeBounds<usize>,
        remote_mr_range: impl RangeBounds<usize>,
        imm_data: Option<u32>,
    ) -> Result<impl WorkRequest, std::io::Error> {
        let wr_id = self.next_wr_id.fetch_add(1, Ordering::Relaxed);
        self.qp.post_write(
            &[self.mr.slice(mr_range)],
            self.remote_mr.slice(remote_mr_range),
            wr_id,
            imm_data,
        )?;
        Ok(CachedWorkRequest::new(wr_id, self.cached_cq.clone()))
    }

    /// TODO: WRITE SAFETY
    unsafe fn post_read(
        &mut self,
        mr_range: impl RangeBounds<usize>,
        remote_mr_slice: impl RangeBounds<usize>,
    ) -> Result<impl WorkRequest, std::io::Error> {
        let wr_id = self.next_wr_id.fetch_add(1, Ordering::Relaxed);
        self.qp.post_read(
            &[self.mr.slice(mr_range)],
            self.remote_mr.slice(remote_mr_slice),
            wr_id,
        )?;
        Ok(CachedWorkRequest::new(wr_id, self.cached_cq.clone()))
    }
}

#[repr(u8)]
#[derive(Debug, Default, Copy, Clone)]
enum RendezvousState {
    #[default]
    Waiting,
    Ready,
}

#[repr(transparent)]
#[derive(Debug)]
struct RendezvousMemoryRegion([RendezvousState; 2]);

impl RendezvousMemoryRegion {
    fn new() -> Self {
        Self([RendezvousState::Ready, RendezvousState::Waiting])
    }

    fn remote_state(&self) -> RendezvousState {
        self.0[1]
    }

    fn reset_remote_state(&mut self) {
        self.0[1] = RendezvousState::Waiting;
    }

    fn remote_state_range(&self) -> Range<usize> {
        1..2
    }

    fn local_state_range(&self) -> Range<usize> {
        0..1
    }
}

impl Deref for RendezvousMemoryRegion {
    type Target = [RendezvousState];

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl<const CQ_SIZE: usize> RdmaRendezvous for IBSimpleUnit<CQ_SIZE> {
    fn rendezvous(&mut self) -> std::io::Result<()> {
        // Write READY to the peer's rendezvous memory
        let wr_id = self.next_wr_id.fetch_add(1, Ordering::Relaxed);
        self.qp.post_write(
            &[self
                .rendezvous_mr
                .slice(self.rendezvous_state.local_state_range())],
            self.remote_rendezvous_mr
                .slice(self.rendezvous_state.remote_state_range()),
            wr_id,
            None,
        )?;
        CachedWorkRequest::new(wr_id, self.cached_cq.clone()).wait()?;

        // Wait for peer to write on our rendezvous memory
        while let RendezvousState::Waiting = self.rendezvous_state.remote_state() {
            std::hint::spin_loop();
        }

        // Reset our rendezvous memory so the operation can be repeated
        self.rendezvous_state.reset_remote_state();

        Ok(())
    }

    fn rendezvous_timeout(&mut self, timeout: Duration) -> std::io::Result<()> {
        todo!()
    }
}
