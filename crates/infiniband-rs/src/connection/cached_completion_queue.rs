use crate::ibverbs::completion_queue::IbvCompletionQueue;
use ibverbs_sys::*;
use intmap::IntMap;
use std::io;

#[derive(Debug)]
pub struct IbvCachedCompletionQueue {
    cq: IbvCompletionQueue,
    cache: IntMap<u64, ibv_wc>,
    poll_buf: Vec<ibv_wc>,
}

impl IbvCachedCompletionQueue {
    /// Wrapper over a completion queue that adds a cache to polled data.
    /// Allows checking for completion without consuming all work completions.
    pub(super) fn wrap_cq(cq: IbvCompletionQueue) -> Self {
        let poll_buf_length = cq.min_capacity() as usize;
        Self {
            cq,
            cache: IntMap::new(),
            poll_buf: vec![ibv_wc::default(); poll_buf_length],
        }
    }

    /// Polls work completions into the cache.
    /// Returns the number of new work completions polled.
    pub fn poll(&mut self) -> io::Result<usize> {
        // Poll the cq for new work completions
        let wc_slice = self.cq.poll(self.poll_buf.as_mut_slice())?;

        // Fill cache with polled work completions
        for wc in wc_slice.iter() {
            self.cache.insert(wc.wr_id(), *wc);
        }

        Ok(wc_slice.len())
    }

    /// Consume a cached work completion.
    /// Returns Some if cached, None if not.
    /// Removes the work completion from the cache.
    pub fn consume(&mut self, wr_id: u64) -> Option<ibv_wc> {
        self.cache.remove(wr_id)
    }
}
