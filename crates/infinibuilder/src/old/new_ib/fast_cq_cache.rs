use ibverbs::{CompletionQueue, ibv_wc};
use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct CachedCompletionQueue<const N: usize> {
    cq: Arc<CompletionQueue>,
    cache: UnsafeCell<[Option<ibv_wc>; N]>,
    next_wr_id: AtomicU64,
}

impl<const N: usize> CachedCompletionQueue<N> {
    pub fn reserve_wr_id(&self) -> u64 {
        let next_wr_id = self.next_wr_id.fetch_add(1, Ordering::Relaxed);

        if unsafe { self.cache()[next_wr_id as usize] }.is_some() {
            panic!("Cache overflow");
        }

        next_wr_id % N as u64
    }

    /// SAFETY: This is only thread safe if the reserved wr ids are respected and used only once.
    /// Otherwise, this method could be called concurrently by two threads which write to the same
    /// cache slot. If respected, this will not happen as each, work completion is guaranteed by the
    /// verbs api to be poll thread safe.
    pub unsafe fn update_cache(&self) -> std::io::Result<usize> {
        let mut poll_buff: [ibv_wc; N] = unsafe { MaybeUninit::uninit().assume_init() };
        let wc_slice = self.cq.poll(&mut poll_buff)?;

        // Fill cache with polled work completions
        for wc in wc_slice.iter() {
            self.mut_cache().cache[wc.wr_id() as usize] = Some(*wc);
        }

        Ok(wc_slice.len())
    }

    /// SAFETY: The returned work completion option might be overwritten by a cache update while its
    /// copied. The chances of this are minimized by making the space of work ids larger (N) as enough
    /// work completions have to be reserved and completed to wraparound and reach the returned one again.
    /// If the output is Some(ibv_wc), the completion is consumed (set to None on the cache).
    pub unsafe fn consume_wc(&self, wr_id: u64) -> Option<ibv_wc> {
        self.mut_cache()[wr_id as usize].take()
    }

    unsafe fn mut_cache(&self) -> &mut [Option<ibv_wc>; N] {
        unsafe { &mut *self.cache.get() }
    }

    unsafe fn cache(&self) -> &[Option<ibv_wc>; N] {
        unsafe { &*self.cache.get() }
    }
}
