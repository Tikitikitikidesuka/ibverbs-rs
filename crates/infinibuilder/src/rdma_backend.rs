use ibverbs::{CompletionQueue, MemoryRegion, ProtectionDomain, QueuePair, ibv_wc, RemoteMemorySlice};
use std::ops::RangeBounds;

pub struct RDMABackend<'a> {
    pd: ProtectionDomain,
    cq: CompletionQueue,
    mrs: Vec<MemoryRegion<&'a [u8]>>,
    qps: Vec<QueuePair>,
}

impl RDMABackend {
    pub fn post_send(
        &mut self,
        qp_idx: usize,
        mr_idx: usize,
        slice_range: impl RangeBounds<usize>,
    ) -> std::io::Result<WorkRequest> {
        todo!()
    }

    pub fn post_receive(
        &mut self,
        qp_idx: usize,
        mr_idx: usize,
        slice_range: impl RangeBounds<usize>,
    ) -> std::io::Result<WorkRequest> {
        todo!()
    }

    pub fn post_read(
        &mut self,
        qp_idx: usize,
        mr_idx: usize,
        slice_range: impl RangeBounds<usize>,
    ) -> std::io::Result<WorkRequest> {
    }
}

pub struct WorkRequest {}

pub enum WorkRequestStatus<T> {
    Pending,
    Done(T),
}

impl WorkRequest {
    fn poll(&self) -> std::io::Result<WorkRequestStatus<ibv_wc>> {
        todo!()
    }

    fn wait_barrier(self) -> std::io::Result<ibv_wc> {
        use WorkRequestStatus::*;

        loop {
            match self.poll()? {
                Pending => std::hint::spin_loop(),
                Done(wc) => return Ok(wc),
            }
        }
    }
}
