use crate::ibverbs::remote_memory_region::{
    RemoteMemoryRegion, RemoteMemorySlice, RemoteMemorySliceMut,
};
use std::borrow::{Borrow, BorrowMut};
use std::marker::PhantomData;
use std::ops::RangeBounds;

pub struct RankRemoteMemoryRegion {
    peer: usize,
    remote_mr: RemoteMemoryRegion,
}

impl RankRemoteMemoryRegion {
    pub(super) fn new(peer: usize, remote_mr: RemoteMemoryRegion) -> Self {
        Self { peer, remote_mr }
    }

    pub fn peer(&self) -> usize {
        self.peer
    }

    pub fn len(&self) -> usize {
        self.remote_mr.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn as_slice(&'_ self) -> RankRemoteMemorySlice<'_> {
        RankRemoteMemorySlice {
            peer: self.peer,
            slice: self.remote_mr.as_slice(),
        }
    }

    pub fn as_slice_mut(&'_ mut self) -> RankRemoteMemorySliceMut<'_> {
        RankRemoteMemorySliceMut {
            peer: self.peer,
            slice: self.remote_mr.as_slice_mut(),
        }
    }

    pub fn slice(&'_ self, range: impl RangeBounds<usize>) -> Option<RankRemoteMemorySlice<'_>> {
        Some(RankRemoteMemorySlice {
            peer: self.peer,
            slice: self.remote_mr.slice(range)?,
        })
    }

    pub fn slice_mut(
        &'_ mut self,
        range: impl RangeBounds<usize>,
    ) -> Option<RankRemoteMemorySliceMut<'_>> {
        Some(RankRemoteMemorySliceMut {
            peer: self.peer,
            slice: self.remote_mr.slice_mut(range)?,
        })
    }
}

#[derive(Debug)]
pub struct RankRemoteMemorySlice<'a> {
    pub(super) peer: usize,
    pub(super) slice: RemoteMemorySlice<'a>,
}

impl<'a> Borrow<RemoteMemorySlice<'a>> for RankRemoteMemorySlice<'a> {
    fn borrow(&self) -> &RemoteMemorySlice<'a> {
        &self.slice
    }
}

#[derive(Debug)]
pub struct RankRemoteMemorySliceMut<'a> {
    pub(super) peer: usize,
    pub(super) slice: RemoteMemorySliceMut<'a>,
}

impl<'a> Borrow<RemoteMemorySliceMut<'a>> for RankRemoteMemorySliceMut<'a> {
    fn borrow(&self) -> &RemoteMemorySliceMut<'a> {
        &self.slice
    }
}

impl<'a> BorrowMut<RemoteMemorySliceMut<'a>> for RankRemoteMemorySliceMut<'a> {
    fn borrow_mut(&mut self) -> &mut RemoteMemorySliceMut<'a> {
        &mut self.slice
    }
}

impl<'a> RankRemoteMemorySlice<'a> {
    pub fn peer(&self) -> usize {
        self.peer
    }

    pub fn len(&self) -> usize {
        self.slice.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn slice(&'_ self, range: impl RangeBounds<usize>) -> Option<RankRemoteMemorySlice<'_>> {
        Some(RankRemoteMemorySlice {
            peer: self.peer,
            slice: self.slice.slice(range)?,
        })
    }

    pub fn split_at(
        &'_ self,
        mid: usize,
    ) -> (RankRemoteMemorySlice<'_>, RankRemoteMemorySlice<'_>) {
        let (a, b) = self.slice.split_at(mid);
        (
            RankRemoteMemorySlice {
                peer: self.peer,
                slice: a,
            },
            RankRemoteMemorySlice {
                peer: self.peer,
                slice: b,
            },
        )
    }

    pub fn split_at_checked(
        &'_ self,
        mid: usize,
    ) -> Option<(RankRemoteMemorySlice<'_>, RankRemoteMemorySlice<'_>)> {
        let (a, b) = self.slice.split_at_checked(mid)?;
        Some((
            RankRemoteMemorySlice {
                peer: self.peer,
                slice: a,
            },
            RankRemoteMemorySlice {
                peer: self.peer,
                slice: b,
            },
        ))
    }
}

impl<'a> RankRemoteMemorySliceMut<'a> {
    pub fn peer(&self) -> usize {
        self.peer
    }

    pub fn len(&self) -> usize {
        self.slice.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slice.is_empty()
    }

    pub fn slice(&'_ self, range: impl RangeBounds<usize>) -> Option<RankRemoteMemorySlice<'_>> {
        Some(RankRemoteMemorySlice {
            peer: self.peer,
            slice: self.slice.slice(range)?,
        })
    }

    pub fn slice_mut(
        &'_ mut self,
        range: impl RangeBounds<usize>,
    ) -> Option<RankRemoteMemorySliceMut<'_>> {
        Some(RankRemoteMemorySliceMut {
            peer: self.peer,
            slice: self.slice.slice_mut(range)?,
        })
    }

    pub fn split_at(
        &'_ self,
        mid: usize,
    ) -> (RankRemoteMemorySlice<'_>, RankRemoteMemorySlice<'_>) {
        let (a, b) = self.slice.split_at(mid);
        (
            RankRemoteMemorySlice {
                peer: self.peer,
                slice: a,
            },
            RankRemoteMemorySlice {
                peer: self.peer,
                slice: b,
            },
        )
    }

    pub fn split_at_checked(
        &'_ self,
        mid: usize,
    ) -> Option<(RankRemoteMemorySlice<'_>, RankRemoteMemorySlice<'_>)> {
        let (a, b) = self.slice.split_at_checked(mid)?;
        Some((
            RankRemoteMemorySlice {
                peer: self.peer,
                slice: a,
            },
            RankRemoteMemorySlice {
                peer: self.peer,
                slice: b,
            },
        ))
    }

    pub fn split_at_mut(
        &'_ mut self,
        mid: usize,
    ) -> (RankRemoteMemorySliceMut<'_>, RankRemoteMemorySliceMut<'_>) {
        let (a, b) = self.slice.split_at_mut(mid);
        (
            RankRemoteMemorySliceMut {
                peer: self.peer,
                slice: a,
            },
            RankRemoteMemorySliceMut {
                peer: self.peer,
                slice: b,
            },
        )
    }

    pub fn split_at_mut_checked(
        &'_ mut self,
        mid: usize,
    ) -> Option<(RankRemoteMemorySliceMut<'_>, RankRemoteMemorySliceMut<'_>)> {
        let (a, b) = self.slice.split_at_mut_checked(mid)?;
        Some((
            RankRemoteMemorySliceMut {
                peer: self.peer,
                slice: a,
            },
            RankRemoteMemorySliceMut {
                peer: self.peer,
                slice: b,
            },
        ))
    }
}
