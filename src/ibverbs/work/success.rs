/// Represents the successful completion of a Work Request.
///
/// This struct contains metadata returned by the hardware after an operation finishes successfully.
/// It is typically extracted from a [`WorkCompletion`](crate::ibverbs::work::WorkCompletion).
#[derive(Copy, Clone, Debug)]
pub struct WorkSuccess {
    imm_data: Option<u32>,
    gathered_length: usize,
}

impl WorkSuccess {
    pub(super) fn new(imm_data: Option<u32>, gathered_length: usize) -> Self {
        Self {
            imm_data,
            gathered_length,
        }
    }
}

impl WorkSuccess {
    /// Returns the immediate data (if any) received with this completion.
    ///
    /// This value is automatically converted from network byte order (Big Endian)
    /// to the host's native byte order.
    ///
    /// Present if the sender used [`SendWorkRequest::with_immediate`](crate::ibverbs::work::SendWorkRequest::with_immediate)
    /// or [`WriteWorkRequest::with_immediate`](crate::ibverbs::work::WriteWorkRequest::with_immediate).
    pub fn immediate_data(&self) -> Option<u32> {
        self.imm_data.map(u32::from_be)
    }

    /// Returns the number of local bytes modified by this operation.
    ///
    /// * **Receive** — The total bytes written into the local Scatter buffers.
    /// * **RDMA Read** — The total bytes fetched from remote memory and written to local Scatter buffers.
    /// * **Send / RDMA Write** — Zero (these operations do not modify local memory during completion).
    pub fn gathered_length(&self) -> usize {
        self.gathered_length
    }
}
