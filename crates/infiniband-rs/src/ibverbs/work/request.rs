use crate::ibverbs::memory::{GatherElement, RemoteMemoryRegion, ScatterElement};

/// A request to send data to a remote peer.
///
/// In a Send operation, the local node pushes data to a remote node. The remote node must
/// have posted a corresponding [`ReceiveWorkRequest`] to accept the data.
///
/// # Lifetimes
///
/// *   `'wr`: The lifetime of this struct. It must live until the request is posted to the Queue Pair.
/// *   `'data`: The lifetime of the local data buffer. It is tied to the [`GatherElement`]
///     and must remain valid until the operation completes.
#[derive(Debug, Clone)]
pub struct SendWorkRequest<'wr, 'data> {
    pub(crate) gather_elements: &'wr [GatherElement<'data>],
    pub(crate) imm_data: Option<u32>,
}

/// A request to receive data from a remote peer.
///
/// In a Receive operation, the local node provides a buffer to store incoming data sent by
/// a remote node. This request must be posted *before* the incoming message arrives.
///
/// # Lifetimes
///
/// *   `'wr`: The lifetime of this struct. It must live until the request is posted to the Queue Pair.
/// *   `'data`: The lifetime of the local data buffer. It is tied to the [`ScatterElement`]
///     and must remain valid until the operation completes.
#[derive(Debug)]
pub struct ReceiveWorkRequest<'wr, 'data> {
    pub(crate) scatter_elements: &'wr mut [ScatterElement<'data>],
}

/// A request to write data directly into remote memory.
///
/// This operation copies data from local memory (Source) to a specific address in remote
/// memory (Destination). The remote CPU is not involved.
///
/// # Lifetimes
///
/// *   `'wr`: The lifetime of this struct. It must live until the request is posted to the Queue Pair.
/// *   `'data`: The lifetime of the local data buffer. It is tied to the [`GatherElement`]
///     and must remain valid until the operation completes.
#[derive(Debug, Clone)]
pub struct WriteWorkRequest<'wr, 'data> {
    pub(crate) gather_elements: &'wr [GatherElement<'data>],
    pub(crate) remote_mr: RemoteMemoryRegion,
    pub(crate) imm_data: Option<u32>,
}

/// A request to read data directly from remote memory.
///
/// This operation fetches data from a specific address in remote memory (Source) and
/// writes it to local memory (Destination).
///
/// # Lifetimes
///
/// *   `'wr`: The lifetime of this struct. It must live until the request is posted to the Queue Pair.
/// *   `'data`: The lifetime of the local data buffer. It is tied to the [`ScatterElement`]
///     and must remain valid until the operation completes.
#[derive(Debug)]
pub struct ReadWorkRequest<'wr, 'data> {
    pub(crate) scatter_elements: &'wr mut [ScatterElement<'data>],
    pub(crate) remote_mr: RemoteMemoryRegion,
}

impl<'wr, 'data> SendWorkRequest<'wr, 'data> {
    /// Creates a new Send request using the provided list of gather elements.
    pub fn new(gather_elements: &'wr [GatherElement<'data>]) -> Self {
        Self {
            gather_elements,
            imm_data: None,
        }
    }

    /// Attach immediate data (a 32-bit integer) to the operation.
    pub fn with_immediate(mut self, imm_data: u32) -> Self {
        self.imm_data = Some(imm_data);
        self
    }

    /// Creates a new Send request containing only immediate data (0-byte payload).
    pub fn only_immediate(imm_data: u32) -> Self {
        Self {
            gather_elements: &[],
            imm_data: Some(imm_data),
        }
    }
}

impl<'wr, 'data> ReceiveWorkRequest<'wr, 'data> {
    /// Creates a new Receive request using the provided list of scatter elements.
    pub fn new(scatter_elements: &'wr mut [ScatterElement<'data>]) -> Self {
        Self { scatter_elements }
    }
}

impl<'wr, 'data> WriteWorkRequest<'wr, 'data> {
    /// Creates a new RDMA Write request.
    ///
    /// *   `gather_elements`: The local source data to write.
    /// *   `remote_slice`: The remote destination memory region.
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

    /// Attaches 32 bits of immediate data to the write operation.
    ///
    /// The immediate value is delivered to the remote peer via a completion notification.
    /// **The remote peer must have posted a [`ReceiveWorkRequest`] to capture this.**
    pub fn with_immediate(mut self, imm_data: u32) -> Self {
        self.imm_data = Some(imm_data);
        self
    }
}

impl<'wr, 'data> ReadWorkRequest<'wr, 'data> {
    /// Creates a new RDMA Read request.
    ///
    /// *   `scatter_elements`: The local destination buffer for the read data.
    /// *   `remote_mr`: The remote source memory region.
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
