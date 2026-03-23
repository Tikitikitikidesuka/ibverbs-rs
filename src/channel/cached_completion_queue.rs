use crate::ibverbs::completion_queue::{CompletionQueue, PollSlot};
use crate::ibverbs::error::IbvResult;
use crate::ibverbs::work::WorkCompletion;
use intmap::IntMap;

/// A [`CompletionQueue`] wrapper that caches polled work completions.
///
/// When multiple work requests share a single completion queue, polling for one
/// may return completions for others. `CachedCompletionQueue` stores these extra
/// completions so they can be retrieved later by work request ID without re-polling
/// the hardware.
#[derive(Debug)]
pub struct CachedCompletionQueue {
    cq: CompletionQueue,
    cache: IntMap<u64, WorkCompletion>,
    poll_buf: Vec<PollSlot>,
}

impl CachedCompletionQueue {
    /// Wraps a [`CompletionQueue`] with an in-memory completion cache.
    pub(super) fn wrap_cq(cq: CompletionQueue) -> Self {
        let poll_buf_length = cq.min_capacity() as usize;
        Self {
            cq,
            cache: IntMap::new(),
            poll_buf: vec![PollSlot::default(); poll_buf_length],
        }
    }

    /// Polls the completion queue and stores any new completions in the cache.
    ///
    /// Returns the number of new completions polled.
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

    /// Removes and returns a cached completion for the given work request ID, if present.
    pub fn consume(&mut self, wr_id: u64) -> Option<WorkCompletion> {
        self.cache.remove(wr_id)
    }
}
