use derivative::Derivative;
use ibverbs::{CompletionQueue, ibv_wc};
use intmap::IntMap;
use std::cmp::min;
use thiserror::Error;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct CachedCompletionQueue {
    #[derivative(Debug = "ignore")]
    cq: CompletionQueue,
    cq_cache: IntMap<u64, ibv_wc>,
    ignore_wr_ids: IntMap<u64, ()>,
    capacity: usize,
}

#[derive(Debug, Error)]
pub enum PollError {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error("Completion queue cache is full")]
    CacheFull,
}

impl CachedCompletionQueue {
    pub fn new(cq: CompletionQueue, capacity: usize) -> Self {
        Self {
            cq,
            cq_cache: IntMap::default(),
            ignore_wr_ids: IntMap::default(),
            capacity,
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn available_space(&self) -> usize {
        self.capacity - self.cq_cache.len()
    }

    /// Polls work completions into the cache.
    pub fn poll<const POLL_BUFF_SIZE: usize>(&mut self) -> Result<usize, PollError> {
        // Check available space on cache
        let available_space = self.capacity - self.cq_cache.len();
        if available_space == 0 {
            return Err(PollError::CacheFull);
        }

        // Poll the cq for new work completions
        let poll_limit = min(POLL_BUFF_SIZE, available_space);
        let mut poll_buff = [ibv_wc::default(); POLL_BUFF_SIZE];
        let wc_slice = self.cq.poll(&mut poll_buff[..poll_limit])?;

        // Fill cache with polled work completions
        for wc in wc_slice.iter() {
            if self.ignore_wr_ids.remove(wc.wr_id()).is_none() {
                // If not in the ignore set, insert to cache
                self.cq_cache.insert(wc.wr_id(), *wc);
            }
        }

        Ok(wc_slice.len())
    }

    pub fn consume(&mut self, wr_id: u64) -> Option<ibv_wc> {
        self.cq_cache.remove(wr_id)
    }

    pub fn ignore(&mut self, wr_id: u64) {
        self.ignore_wr_ids.insert(wr_id, ());
    }
}
