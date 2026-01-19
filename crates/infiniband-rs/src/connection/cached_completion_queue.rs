use crate::ibverbs::completion_queue::{IbvCompletionQueue, IbvCompletionQueuePollSlot};
use crate::ibverbs::work_completion::IbvWorkCompletion;
use intmap::IntMap;
use std::io;

#[derive(Debug)]
pub struct IbvCachedCompletionQueue {
    cq: IbvCompletionQueue,
    cache: IntMap<u64, IbvWorkCompletion>,
    poll_buf: Vec<IbvCompletionQueuePollSlot>,
}

impl IbvCachedCompletionQueue {
    /// Wrapper over a completion queue that adds a cache to polled data.
    /// Allows checking for completion without consuming all work completions.
    pub(super) fn wrap_cq(cq: IbvCompletionQueue) -> Self {
        let poll_buf_length = cq.min_capacity() as usize;
        Self {
            cq,
            cache: IntMap::new(),
            poll_buf: vec![IbvCompletionQueuePollSlot::default(); poll_buf_length],
        }
    }

    /// Polls work completions into the cache.
    /// Returns the number of new work completions polled.
    pub fn update(&mut self) -> io::Result<usize> {
        // Poll the cq for new work completions
        let polled_wcs = self.cq.poll(self.poll_buf.as_mut_slice())?;
        let polled_num = polled_wcs.len();

        // Fill cache with polled work completions
        for wc in polled_wcs {
            self.cache.insert(wc.wr_id(), wc);
        }

        Ok(polled_num)
    }

    /// Returns Some if cached, None if not.
    pub fn poll(&mut self, wr_id: u64) -> Option<IbvWorkCompletion> {
        self.cache.get(wr_id).copied()
    }

    /// Consume a cached work completion.
    /// Returns Some if cached, None if not.
    /// Removes the work completion from the cache.
    pub fn consume(&mut self, wr_id: u64) -> Option<IbvWorkCompletion> {
        self.cache.remove(wr_id)
    }
}
