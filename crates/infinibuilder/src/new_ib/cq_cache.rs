use dashmap::DashMap;
use ibverbs::{CompletionQueue, ibv_wc};
use std::mem::MaybeUninit;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct CachedCompletionQueue<const CQ_SIZE: usize> {
    cq: Arc<CompletionQueue>,
    cache: DashMap<u64, ibv_wc>,
    next_wr_id: AtomicU64,
}

impl<const CQ_SIZE: usize> CachedCompletionQueue<CQ_SIZE> {
    pub fn new(ib_context: &ibverbs::Context) -> std::io::Result<Self> {
        Ok(Self {
            cq: Arc::new(ib_context.create_cq(CQ_SIZE as i32, 0)?),
            cache: DashMap::new(),
            next_wr_id: AtomicU64::new(0),
        })
    }

    pub fn reserve_wr_id(&self) -> u64 {
        self.next_wr_id.fetch_add(1, Ordering::Relaxed)
    }

    pub fn update_cache(&self) -> std::io::Result<usize> {
        let mut poll_buff: [ibv_wc; CQ_SIZE] = unsafe { MaybeUninit::uninit().assume_init() };
        let wc_slice = self.cq.poll(&mut poll_buff)?;

        // Fill cache with polled work completions
        for wc in wc_slice.iter() {
            self.cache.insert(wc.wr_id(), *wc);
        }

        Ok(wc_slice.len())
    }

    pub fn consume_wc(&self, wr_id: u64) -> Option<ibv_wc> {
        self.cache.remove(&wr_id).map(|(_, wc)| wc)
    }
}
