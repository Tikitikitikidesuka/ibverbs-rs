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
}

impl CachedCompletionQueue {
    /// Wraps a [`CompletionQueue`] with an in-memory completion cache.
    pub fn wrap_cq(cq: CompletionQueue) -> Self {
        Self {
            cq,
            cache: IntMap::new(),
        }
    }

    /// Minimum capacity of the wrapped [`CompletionQueue`]
    pub fn min_capacity(&self) -> u32 {
        self.cq.min_capacity()
    }

    /// Polls the completion queue and stores any new completions in the cache.
    ///
    /// Returns the number of new completions polled.
    pub fn update(&mut self, completions: &mut [PollSlot]) -> IbvResult<usize> {
        // Poll the cq for new work completions
        let polled_wcs = self.cq.poll(completions)?;
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
