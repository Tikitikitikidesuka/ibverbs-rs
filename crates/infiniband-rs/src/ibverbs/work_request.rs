use crate::ibverbs::remote_memory_region::{RemoteMemorySlice, RemoteMemorySliceMut};
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use std::borrow::{Borrow, BorrowMut};
use std::marker::PhantomData;

pub struct SendWorkRequest<'a, E>
where
    E: AsRef<[GatherElement<'a>]>,
{
    pub(super) gather_elements: E,
    pub(super) imm_data: Option<u32>,
    _data_lifetime: PhantomData<GatherElement<'a>>,
}

pub struct ReceiveWorkRequest<'a, E>
where
    E: AsMut<[ScatterElement<'a>]>,
{
    pub(super) scatter_elements: E,
    _data_lifetime: PhantomData<ScatterElement<'a>>,
}

pub struct WriteWorkRequest<'a, E, R>
where
    E: AsRef<[GatherElement<'a>]>,
    R: BorrowMut<RemoteMemorySliceMut<'a>>,
{
    pub(super) gather_elements: E,
    pub(super) remote_slice: R,
    pub(super) imm_data: Option<u32>,
    _gather_elements_lifetime: PhantomData<GatherElement<'a>>,
    _remote_mr_lifetime: PhantomData<RemoteMemorySliceMut<'a>>,
}

pub struct ReadWorkRequest<'a, E, R>
where
    E: AsMut<[ScatterElement<'a>]>,
    R: Borrow<RemoteMemorySlice<'a>>,
{
    pub(super) scatter_elements: E,
    pub(super) remote_slice: R,
    _scatter_elements_lifetime: PhantomData<ScatterElement<'a>>,
    _remote_mr_lifetime: PhantomData<RemoteMemorySlice<'a>>,
}

impl<'a, E> SendWorkRequest<'a, E>
where
    E: AsRef<[GatherElement<'a>]>,
{
    pub fn new(gather_elements: E) -> Self {
        Self {
            gather_elements,
            imm_data: None,
            _data_lifetime: PhantomData,
        }
    }

    pub fn with_immediate(mut self, imm_data: u32) -> Self {
        self.imm_data = Some(imm_data);
        self
    }
}

impl<'a> SendWorkRequest<'a, &[GatherElement<'a>]> {
    pub fn only_immediate(imm_data: u32) -> Self {
        Self {
            gather_elements: &[],
            imm_data: Some(imm_data),
            _data_lifetime: PhantomData,
        }
    }
}

impl<'a, E> ReceiveWorkRequest<'a, E>
where
    E: AsMut<[ScatterElement<'a>]>,
{
    pub fn new(scatter_elements: E) -> Self {
        Self {
            scatter_elements,
            _data_lifetime: PhantomData,
        }
    }
}

impl<'a, E, R> WriteWorkRequest<'a, E, R>
where
    E: AsRef<[GatherElement<'a>]>,
    R: BorrowMut<RemoteMemorySliceMut<'a>>,
{
    pub fn new(gather_elements: E, remote_slice: R) -> Self {
        Self {
            gather_elements,
            remote_slice,
            imm_data: None,
            _gather_elements_lifetime: PhantomData,
            _remote_mr_lifetime: PhantomData,
        }
    }

    pub fn with_immediate(mut self, imm_data: u32) -> Self {
        self.imm_data = Some(imm_data);
        self
    }
}

impl<'a, E, R> ReadWorkRequest<'a, E, R>
where
    E: AsMut<[ScatterElement<'a>]>,
    R: Borrow<RemoteMemorySlice<'a>>,
{
    pub fn new(scatter_elements: E, remote_slice: R) -> Self {
        Self {
            scatter_elements,
            remote_slice,
            _scatter_elements_lifetime: Default::default(),
            _remote_mr_lifetime: Default::default(),
        }
    }
}
