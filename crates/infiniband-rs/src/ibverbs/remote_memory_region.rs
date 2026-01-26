use crate::ibverbs::protection_domain::ProtectionDomainInner;
use ibverbs_sys::ibv_mr;
use std::marker::PhantomData;
use std::ops::{Bound, Index, Range, RangeBounds};
use std::sync::Arc;

#[derive(Debug)]
pub struct RemoteMemoryRegion {
    pd: Arc<ProtectionDomainInner>,
    mr: *mut ibv_mr,
}

#[derive(Debug, Copy, Clone)]
pub struct RemoteMemorySlice<'a> {
    addr: usize,
    length: usize,
    rkey: u32,
    // SAFETY INVARIANT: SGE cannot outlive the referenced remote memory region
    _mr_lifetime: PhantomData<&'a RemoteMemoryRegion>,
}

#[derive(Debug)]
pub struct RemoteMemorySliceMut<'a> {
    addr: usize,
    length: usize,
    rkey: u32,
    // SAFETY INVARIANT: SGE cannot outlive the referenced remote memory region
    _mr_lifetime: PhantomData<&'a mut RemoteMemoryRegion>,
}

impl RemoteMemoryRegion {
    pub fn len(&self) -> usize {
        unsafe { *self.mr }.length
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn slice(&self, range: impl RangeBounds<usize>) -> Option<RemoteMemorySlice> {
        let range = normalize_range(self.len(), range)?;
        let base_addr = unsafe { (*self.mr).addr as usize };

        Some(RemoteMemorySlice {
            addr: base_addr + range.start,
            length: range.len(),
            rkey: unsafe { (*self.mr).rkey },
            _mr_lifetime: PhantomData,
        })
    }

    pub fn slice_mut(&mut self, range: impl RangeBounds<usize>) -> Option<RemoteMemorySliceMut> {
        let range = normalize_range(self.len(), range)?;
        let base_addr = unsafe { (*self.mr).addr as usize };

        Some(RemoteMemorySliceMut {
            addr: base_addr + range.start,
            length: range.len(),
            rkey: unsafe { (*self.mr).rkey },
            _mr_lifetime: Default::default(),
        })
    }
}

impl<'a> RemoteMemorySlice<'a> {
    pub fn len(&self) -> usize {
        self.length
    }

    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    pub fn split_at(&self, mid: usize) -> (RemoteMemorySlice, RemoteMemorySlice) {
        match self.split_at_checked(mid) {
            Some(pair) => pair,
            None => panic!("mid > len"),
        }
    }

    pub fn split_at_checked(&self, mid: usize) -> Option<(RemoteMemorySlice, RemoteMemorySlice)> {
        if mid > self.len() {
            return None;
        }

        Some((
            RemoteMemorySlice {
                addr: self.addr,
                length: mid,
                rkey: self.rkey,
                _mr_lifetime: PhantomData,
            },
            RemoteMemorySlice {
                addr: self.addr + mid,
                length: self.length - mid,
                rkey: self.rkey,
                _mr_lifetime: PhantomData,
            },
        ))
    }

    pub fn slice(&self, range: impl RangeBounds<usize>) -> Option<RemoteMemorySlice<'a>> {
        let range = normalize_range(self.length, range)?;

        Some(RemoteMemorySlice {
            addr: self.addr + range.start,
            length: range.len(),
            rkey: self.rkey,
            _mr_lifetime: PhantomData,
        })
    }
}

impl<'a> RemoteMemorySliceMut<'a> {
    pub fn len(&self) -> usize {
        self.length
    }

    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    pub fn split_at_mut(&mut self, mid: usize) -> (RemoteMemorySliceMut, RemoteMemorySliceMut) {
        match self.split_at_mut_checked(mid) {
            Some(pair) => pair,
            None => panic!("mid > len"),
        }
    }

    pub fn split_at_mut_checked(
        &mut self,
        mid: usize,
    ) -> Option<(RemoteMemorySliceMut, RemoteMemorySliceMut)> {
        if mid > self.len() {
            return None;
        }

        Some((
            RemoteMemorySliceMut {
                addr: self.addr,
                length: mid,
                rkey: self.rkey,
                _mr_lifetime: PhantomData,
            },
            RemoteMemorySliceMut {
                addr: self.addr + mid,
                length: self.length - mid,
                rkey: self.rkey,
                _mr_lifetime: PhantomData,
            },
        ))
    }

    pub fn slice_mut(&mut self, range: impl RangeBounds<usize>) -> Option<RemoteMemorySliceMut> {
        let range = normalize_range(self.length, range)?;

        Some(RemoteMemorySliceMut {
            addr: self.addr + range.start,
            length: range.len(),
            rkey: self.rkey,
            _mr_lifetime: PhantomData,
        })
    }
}

/// Normalize a range relative to a memory's length.
/// Returns Some(start..end) if valid, None if out of bounds.
fn normalize_range(memory_length: usize, range: impl RangeBounds<usize>) -> Option<Range<usize>> {
    let start = match range.start_bound() {
        Bound::Included(&n) => n,
        Bound::Excluded(&n) => n.checked_add(1)?,
        Bound::Unbounded => 0,
    };

    let end = match range.end_bound() {
        Bound::Included(&n) => n.checked_add(1)?,
        Bound::Excluded(&n) => n,
        Bound::Unbounded => memory_length,
    };

    if start > end || end > memory_length {
        None
    } else {
        Some(start..end)
    }
}
