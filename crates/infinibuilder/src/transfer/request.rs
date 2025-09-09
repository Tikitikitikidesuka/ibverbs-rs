use std::sync::Arc;
use dashmap::DashMap;
use ibverbs::{CompletionQueue, ibv_wc};

pub enum TransferRequestStatus<T> {
    Pending,
    Done(T),
}

pub struct TransferRequest {
    wr_id: u64,
    cq: Arc<CompletionQueue>,
    completion_cache: Arc<DashMap<u64, ibv_wc>>,
}

impl TransferRequest {
    const POLL_BUFFER_LENGTH: usize = 32;

    pub fn new(
        wr_id: u64,
        cq: Arc<CompletionQueue>,
        completion_cache: Arc<DashMap<u64, ibv_wc>>,
    ) -> Self {
        Self {
            wr_id,
            cq,
            completion_cache,
        }
    }

    pub fn poll(&self) -> std::io::Result<TransferRequestStatus<ibv_wc>> {
        let mut cq_buff = [ibv_wc::default(); Self::POLL_BUFFER_LENGTH];

        let wc_slice = self.cq.poll(&mut cq_buff)?;
        wc_slice.iter().for_each(|&wc| {
            self.completion_cache.insert(wc.wr_id(), wc);
        });

        match self.completion_cache.remove(&self.wr_id) {
            Some((_, wc)) => Ok(TransferRequestStatus::Done(wc)),
            None => Ok(TransferRequestStatus::Pending),
        }
    }

    pub fn wait(self) -> std::io::Result<ibv_wc>
    where
        Self: Sized,
    {
        use TransferRequestStatus::*;

        loop {
            match self.poll()? {
                Pending => std::hint::spin_loop(),
                Done(wc) => return Ok(wc),
            }
        }
    }
}
