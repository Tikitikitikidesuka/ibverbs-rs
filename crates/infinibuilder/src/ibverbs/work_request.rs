use crate::ibverbs::cached_cq::CachedCompletionQueue;
use crate::ibverbs::ibv_wc_conversion::work_completion_from_ibv_wc;
use crate::rdma_traits::{WorkCompletion, WorkRequest};
use ibverbs::ibv_wc;
use std::sync::Arc;
use std::time::Duration;

pub struct CachedWorkRequest<const CQ_SIZE: usize> {
    wr_id: u64,
    cq_cache: Arc<CachedCompletionQueue<CQ_SIZE>>,
    opt_wc: Option<ibv_wc>,
}

impl<const CQ_SIZE: usize> WorkRequest for CachedWorkRequest<CQ_SIZE> {
    fn poll(&mut self) -> std::io::Result<Option<WorkCompletion>> {
        self._update_from_all()?
            .map(|wc| work_completion_from_ibv_wc(wc))
            .transpose()
    }

    fn wait(mut self) -> std::io::Result<WorkCompletion> {
        // Poll all sources first
        if let Some(wc) = self.poll()? {
            return Ok(wc);
        }

        // If not in opt_wc or cache, it will come through cq
        loop {
            // Poll only the completion queue
            self._update_from_cq()?;
            if let Some(wc) = self.opt_wc {
                return work_completion_from_ibv_wc(wc);
            }
            std::hint::spin_loop();
        }
    }

    fn wait_timeout(self, timeout: Duration) -> std::io::Result<WorkCompletion> {
        todo!()
    }
}

impl<const CQ_SIZE: usize> CachedWorkRequest<CQ_SIZE> {
    pub(super) fn new(wr_id: u64, cq_cache: Arc<CachedCompletionQueue<CQ_SIZE>>) -> Self {
        Self {
            wr_id,
            cq_cache,
            opt_wc: None,
        }
    }

    fn _update_from_all(&mut self) -> std::io::Result<Option<ibv_wc>> {
        // Check if already polled completion
        if let Some(_) = self._update_from_self() {
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

    fn _update_from_self(&self) -> Option<ibv_wc> {
        // Poll self to check if already consumed the completion
        self.opt_wc
    }

    fn _update_from_cq(&mut self) -> std::io::Result<Option<ibv_wc>> {
        // Poll the cq and check the cache
        self.cq_cache.poll()?;
        self._update_from_cache()
    }

    fn _update_from_cache(&mut self) -> std::io::Result<Option<ibv_wc>> {
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
