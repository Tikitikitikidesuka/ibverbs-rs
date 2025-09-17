use crate::new_ib::unsafe_slice::UnsafeSlice;
use crate::new_ib::{SendRecv, WorkRequest, WorkRequestStatus};
use dashmap::DashMap;
use ibverbs::{CompletionQueue, MemoryRegion, ProtectionDomain, QueuePair, ibv_wc};
use static_assertions::const_assert;
use std::collections::HashMap;
use std::fmt::Debug;
use std::io::ErrorKind;
use std::ops::RangeBounds;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

pub struct IbVerbsMultiPeer {
    connections: Vec<Arc<IbVerbsConnection>>,
    mr: MemoryRegion<UnsafeSlice<u8>>,
    pd: ProtectionDomain,
}

pub struct IbVerbsPeer {
    connection: IbVerbsConnection,
    mr: MemoryRegion<UnsafeSlice<u8>>,
    pd: ProtectionDomain,
}

struct IbVerbsConnection {
    qp: QueuePair,
    cq: Arc<CompletionQueue>,
    cq_cache: Arc<CompletionQueueCache<256>>,
}

pub struct IbVerbsTransport<'a> {
    qp: &'a mut QueuePair,
    mr: &'a MemoryRegion<UnsafeSlice<u8>>,
    cq: Arc<CompletionQueue>,
    cq_cache: Arc<CompletionQueueCache<256>>,
}

use CachedWC::*;
use CachedWcNextPtr::*;

#[derive(Debug, Copy, Clone)]
enum CachedWC {
    Uninitialized(CachedWcNextPtr), // Points to index of next free id
    Acquired,
    Set(ibv_wc),
}

#[derive(Debug, Copy, Clone)]
enum CachedWcNextPtr {
    Next(usize),
    Last,
}

struct CompletionQueueCache<const N: usize> {
    inner: Mutex<CompletionQueueCacheInner<N>>,
}

struct CompletionQueueCacheInner<const N: usize> {
    completions: [CachedWC; N],
    next_key: usize,
}

impl<const N: usize> CompletionQueueCache<N> {
    fn new() -> Self {
        // Initialize the free list: each slot points to the next one
        let mut completions = [Uninitialized(Last); N];
        for i in 0..N - 1 {
            completions[i] = Uninitialized(Next(i + 1));
        }

        Self {
            inner: Mutex::new(CompletionQueueCacheInner {
                completions,
                next_key: 0, // Start with first slot
            }),
        }
    }

    // Returns the next unused key
    fn acquire_key(&self) -> Option<usize> {
        // Lock the id head
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let current_head = inner.next_key;

        // Take the head slot
        let head_slot = inner.completions[current_head];

        match head_slot {
            // Slot is free, and more are available -> advance head
            Uninitialized(Next(next_free)) => {
                inner.completions[current_head] = Acquired;
                inner.next_key = next_free;
                Some(current_head)
            }
            // Slot is free, but last one -> hand it out, list now empty
            Uninitialized(Last) => {
                inner.completions[current_head] = Acquired;
                Some(current_head)
            }
            // Already full -> nothing left to give
            _ => None,
        }
    }

    fn release_key(&self, k: u64) -> Result<(), ()> {
        // Check if the id was acquired (not Uninitialized)
        // If it was not, return None
        // If it was:
        // Lock the id head
        // If it points to an Uninitialized, set the release id slot to Uninitialized(Next) pointing to the slot pointed by the head
        //  and set the head to point to the released id slot.
        // If it points to something else than Uninitialized (Meaning there were no free slots), then set the release id slot
        //  to Unitialized(Last) and set the head to point to the released id slot.
    }

    // Sets the value for a wr_id
    fn set_(&self, k: usize, v: ibv_wc) -> Result<(), ()> {
        todo!()
    }
}

impl SendRecv for IbVerbsTransport<'_> {
    type Error = std::io::Error;

    fn post_send(
        &mut self,
        mr_range: impl RangeBounds<usize>,
    ) -> Result<impl WorkRequest, <Self as SendRecv>::Error> {
        // Get wr_id of the future next entry

        unsafe { self.qp.post_send(&[self.mr.slice(mr_range)], wr_id, None) }?;

        Ok(IbVerbsWorkRequest {
            wr_id,
            cq: self.cq.clone(),
            cq_cache: self.cq_cache.clone(),
        })
    }

    fn post_recv(
        &mut self,
        mr_range: impl RangeBounds<usize>,
    ) -> Result<impl WorkRequest, std::io::Error> {
        todo!()
    }
}

pub struct IbVerbsWorkRequest {
    wr_id: u64,
    cq: Arc<CompletionQueue>,
    cq_cache: Arc<Slab<ibv_wc>>,
}

impl IbVerbsWorkRequest {
    const POLL_BUFFER_LENGTH: usize = 32;
}

impl WorkRequest for IbVerbsWorkRequest {
    type WorkCompletion = ibv_wc;
    type WorkRequestError = std::io::Error;

    fn poll(&self) -> Result<WorkRequestStatus<ibv_wc>, std::io::Error> {
        let mut cq_buff = [ibv_wc::default(); Self::POLL_BUFFER_LENGTH];

        let mut wc_slice = self.cq.poll(&mut cq_buff)?;
        while !wc_slice.is_empty() {
            wc_slice.iter().for_each(|&wc| {
                // Insert completion to the cq cache
            });
            wc_slice = self.cq.poll(&mut cq_buff)?;
        }

        // Check if our specific wr is finished
    }
}
