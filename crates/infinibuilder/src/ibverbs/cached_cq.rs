use dashmap::DashMap;
use ibverbs::{CompletionQueue, ibv_wc};
use std::cell::RefCell;
use std::collections::HashMap;
use std::mem::MaybeUninit;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) struct CachedCompletionQueue {
    cq: Arc<CompletionQueue>,
    cq_cache: Rc<RefCell<Vec<Option<ibv_wc>>>>,
    next_wr_id: AtomicU64,
}

pub(super) struct CachedCompletionQueueV2 {
    cq: Arc<CompletionQueue>,
    cq_cache: Rc<RefCell<HashMap<u64, ibv_wc>>>,
    next_wr_id: AtomicU64,
}

pub(super) struct CachedCompletionQueueV1 {
    cq: Arc<CompletionQueue>,
    cq_cache: Arc<DashMap<u64, ibv_wc>>,
    next_wr_id: AtomicU64,
}

unsafe impl Sync for CachedCompletionQueueV1 {}
unsafe impl Send for CachedCompletionQueueV1 {}

impl CachedCompletionQueueV1 {
    const POLL_BUFF_SIZE: usize = 32;

    pub fn new(cq: CompletionQueue) -> Self {
        Self {
            cq: Arc::new(cq),
            cq_cache: Arc::new(DashMap::new()),
            next_wr_id: AtomicU64::new(0),
        }
    }

    pub fn fetch_advance_next_wr_id(&self) -> u64 {
        self.next_wr_id.fetch_add(1, Ordering::Relaxed)
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

impl CachedCompletionQueueV2 {
    const POLL_BUFF_SIZE: usize = 32;

    pub fn new(cq: CompletionQueue) -> Self {
        Self {
            cq: Arc::new(cq),
            cq_cache: Rc::new(RefCell::new(HashMap::new())),
            next_wr_id: AtomicU64::new(0),
        }
    }

    pub fn fetch_advance_next_wr_id(&self) -> u64 {
        self.next_wr_id.fetch_add(1, Ordering::Relaxed)
    }

    pub fn poll<const POLL_BUFF_SIZE: usize>(&self) -> std::io::Result<usize> {
        let mut poll_buff: [ibv_wc; POLL_BUFF_SIZE] =
            unsafe { MaybeUninit::uninit().assume_init() };
        let wc_slice = self.cq.poll(&mut poll_buff)?;

        // Fill cache with polled work completions
        for wc in wc_slice.iter() {
            self.cq_cache.borrow_mut().insert(wc.wr_id(), *wc);
        }

        Ok(wc_slice.len())
    }

    pub fn consume(&self, wr_id: u64) -> Option<ibv_wc> {
        self.cq_cache.borrow_mut().remove(&wr_id)
    }
}

impl CachedCompletionQueue {
    const CACHE_SIZE: usize = 4096;

    pub fn new(cq: CompletionQueue) -> Self {
        Self {
            cq: Arc::new(cq),
            cq_cache: Rc::new(RefCell::new(vec![None; Self::CACHE_SIZE])),
            next_wr_id: AtomicU64::new(0),
        }
    }

    pub fn fetch_advance_next_wr_id(&self) -> u64 {
        self.next_wr_id.fetch_add(1, Ordering::Relaxed)
    }

    pub fn poll<const POLL_BUFF_SIZE: usize>(&self) -> std::io::Result<usize> {
        let mut poll_buff: [ibv_wc; POLL_BUFF_SIZE] =
            unsafe { MaybeUninit::uninit().assume_init() };
        let wc_slice = self.cq.poll(&mut poll_buff)?;

        // Fill cache with polled work completions
        for wc in wc_slice.iter() {
            self.cq_cache.borrow_mut()[wc.wr_id() as usize % Self::CACHE_SIZE] = Some(*wc);
        }

        Ok(wc_slice.len())
    }

    pub fn consume(&self, wr_id: u64) -> Option<ibv_wc> {
        self.cq_cache.borrow_mut()[wr_id as usize % Self::CACHE_SIZE].take()
    }
}
