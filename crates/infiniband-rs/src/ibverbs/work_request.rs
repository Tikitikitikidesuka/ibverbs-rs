use crate::ibverbs::memory::{GatherElement, RemoteMemoryRegion, ScatterElement};

/// 'wr is the lifetime of the work request struct. It lives from when then
/// work request is created until its posted.
/// 'data is the lifetime of the local data referenced by the rdma operation.
/// It is held until the operation completes.
#[derive(Debug, Clone)]
pub struct SendWorkRequest<'wr, 'data> {
    pub(super) gather_elements: &'wr [GatherElement<'data>],
    pub(super) imm_data: Option<u32>,
}

/// 'wr is the lifetime of the work request struct. It lives from when then
/// work request is created until its posted.
/// 'data is the lifetime of the local data referenced by the rdma operation.
/// It is held until the operation completes.
#[derive(Debug)]
pub struct ReceiveWorkRequest<'wr, 'data> {
    pub(super) scatter_elements: &'wr mut [ScatterElement<'data>],
}

/// 'wr is the lifetime of the work request struct. It lives from when then
/// work request is created until its posted.
/// 'data is the lifetime of the local data referenced by the rdma operation.
/// It is held until the operation completes.
#[derive(Debug, Clone)]
pub struct WriteWorkRequest<'wr, 'data> {
    pub(super) gather_elements: &'wr [GatherElement<'data>],
    pub(super) remote_mr: RemoteMemoryRegion,
    pub(super) imm_data: Option<u32>,
}

/// 'wr is the lifetime of the work request struct. It lives from when then
/// work request is created until its posted.
/// 'data is the lifetime of the local data referenced by the rdma operation.
/// It is held until the operation completes.
#[derive(Debug)]
pub struct ReadWorkRequest<'wr, 'data> {
    pub(super) scatter_elements: &'wr mut [ScatterElement<'data>],
    pub(super) remote_mr: RemoteMemoryRegion,
}

impl<'wr, 'data> SendWorkRequest<'wr, 'data> {
    pub fn new(gather_elements: &'wr [GatherElement<'data>]) -> Self {
        Self {
            gather_elements,
            imm_data: None,
        }
    }

    pub fn with_immediate(mut self, imm_data: u32) -> Self {
        self.imm_data = Some(imm_data);
        self
    }

    pub fn only_immediate(imm_data: u32) -> Self {
        Self {
            gather_elements: &[],
            imm_data: Some(imm_data),
        }
    }
}

impl<'wr, 'data> ReceiveWorkRequest<'wr, 'data> {
    pub fn new(scatter_elements: &'wr mut [ScatterElement<'data>]) -> Self {
        Self { scatter_elements }
    }
}

impl<'wr, 'data> WriteWorkRequest<'wr, 'data> {
    pub fn new(
        gather_elements: &'wr [GatherElement<'data>],
        remote_slice: RemoteMemoryRegion,
    ) -> Self {
        Self {
            gather_elements,
            remote_mr: remote_slice,
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
        remote_mr: RemoteMemoryRegion,
    ) -> Self {
        Self {
            scatter_elements,
            remote_mr,
        }
    }
}
