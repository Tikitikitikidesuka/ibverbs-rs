use dashmap::DashMap;
use ibverbs::{CompletionQueue, ibv_wc};
use std::mem::MaybeUninit;
use std::sync::Arc;

pub(super) struct CachedCompletionQueue {
    cq: Arc<CompletionQueue>,
    cq_cache: Arc<DashMap<u64, ibv_wc>>,
}

unsafe impl Sync for CachedCompletionQueue {}
unsafe impl Send for CachedCompletionQueue {}

impl CachedCompletionQueue {
    const POLL_BUFF_SIZE: usize = 32;

    pub fn new(cq: CompletionQueue) -> Self {
        Self {
            cq: Arc::new(cq),
            cq_cache: Arc::new(DashMap::new()),
        }
    }

    pub fn poll<const POLL_BUFF_SIZE: usize>(&self) -> std::io::Result<usize> {
        let mut poll_buff: [ibv_wc; POLL_BUFF_SIZE] =
            unsafe { MaybeUninit::uninit().assume_init() };
        let wc_slice = self.cq.poll(&mut poll_buff)?;

        // Fill cache with polled work completions
        for wc in wc_slice.iter() {
            self.cq_cache.insert(wc.wr_id(), *wc);
        }

        Ok(wc_slice.len())
    }

    pub fn consume(&self, wr_id: u64) -> Option<ibv_wc> {
        self.cq_cache.remove(&wr_id).map(|(_, wc)| wc)
    }
}
