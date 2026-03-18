use crate::ibverbs::completion_queue::{CompletionQueue, PollSlot};
use crate::ibverbs::error::IbvResult;
use crate::ibverbs::work::WorkCompletion;
use intmap::IntMap;

#[derive(Debug)]
pub struct CachedCompletionQueue {
    cq: CompletionQueue,
    cache: IntMap<u64, WorkCompletion>,
    poll_buf: Vec<PollSlot>,
}

impl CachedCompletionQueue {
    /// Wrapper over a completion queue that adds a cache to polled data.
    /// Allows checking for completion without consuming all work completions.
    pub(super) fn wrap_cq(cq: CompletionQueue) -> Self {
        let poll_buf_length = cq.min_capacity() as usize;
        Self {
            cq,
            cache: IntMap::new(),
            poll_buf: vec![PollSlot::default(); poll_buf_length],
        }
    }

    /// Polls work completions into the cache.
    /// Returns the number of new work completions polled.
    pub fn update(&mut self) -> IbvResult<usize> {
        // Poll the cq for new work completions
        let polled_wcs = self.cq.poll(self.poll_buf.as_mut_slice())?;
        let polled_num = polled_wcs.len();

        // Fill cache with polled work completions
        for wc in polled_wcs {
            self.cache.insert(wc.wr_id(), wc);
        }

        Ok(polled_num)
    }

    /// Consume a cached work completion.
    /// Returns Some if cached, None if not.
    /// Removes the work completion from the cache.
    pub fn consume(&mut self, wr_id: u64) -> Option<WorkCompletion> {
        self.cache.remove(wr_id)
    }
}
