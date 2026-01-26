use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
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

/*
pub struct WriteWorkRequest<'wr, 'data> {
    pub(super) gather_elements: &'wr [GatherElement<'data>],
    pub(super) remote_slice: &'wr mut RemoteMemorySliceMut<'data>,
    pub(super) imm_data: Option<u32>,
}

pub struct ReadWorkRequest<'wr, 'data> {
    pub(super) scatter_elements: &'wr mut [ScatterElement<'data>],
    pub(super) remote_slice: &'wr RemoteMemorySlice<'data>,
}
*/

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

/*
impl<'wr, 'data> WriteWorkRequest<'wr, 'data> {
    pub fn new(
        gather_elements: &'wr [GatherElement<'data>],
        remote_slice: &'wr mut RemoteMemorySliceMut<'data>,
    ) -> Self {
        Self {
            gather_elements,
            remote_slice,
            imm_data: None,
        }
    }

    pub fn with_immediate(mut self, imm_data: u32) -> Self {
        self.imm_data = Some(imm_data);
        self
    }
}

impl<'wr, 'data> ReadWorkRequest<'wr, 'data> {
    pub fn new(
        scatter_elements: &'wr mut [ScatterElement<'data>],
        remote_slice: &'wr RemoteMemorySlice<'data>,
    ) -> Self {
        Self {
            scatter_elements,
            remote_slice,
        }
    }
}
 */
