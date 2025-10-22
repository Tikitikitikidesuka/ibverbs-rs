use std::error::Error;
use crate::restructure::ibverbs::work_request::IbvWorkRequest;
use std::ops::RangeBounds;

pub trait RdmaConnection {
    type MR;
    type RemoteMR;
    type WR: RdmaWorkRequest;
    type WC: RdmaWorkCompletion;
    type PostError: Error;

    // Posts a send operation. Will fail if the remote has not posted a receive operation before hand.
    fn post_send(
        &mut self,
        memory_region: &Self::MR,
        memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<Self::WR, Self::PostError>;

    // Posts a receive operation.
    fn post_receive(
        &mut self,
        memory_region: &Self::MR,
        memory_range: impl RangeBounds<usize>,
    ) -> Result<Self::WR, Self::PostError>;

    // Posts a write operation.
    // If sent with immediate data, the data must be obtained in the remote peer
    // by calling post_receive_immediate
    fn post_write(
        &mut self,
        local_memory_region: &Self::MR,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: &Self::RemoteMR,
        remote_memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<Self::WR, Self::PostError>;

    // Posts a read operation.
    fn post_read(
        &mut self,
        local_memory_region: &Self::MR,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: &Self::RemoteMR,
        remote_memory_range: impl RangeBounds<usize>,
    ) -> Result<Self::WR, Self::PostError>;

    fn post_send_immediate_data(
        &mut self,
        immediate_data: u32,
    ) -> Result<IbvWorkRequest, std::io::Error>;

    fn post_receive_immediate_data(&mut self) -> Result<IbvWorkRequest, std::io::Error>;
}

// No traits for QP, PD or CQ as those the user should not care about in this abstraction
// Only things user should interact are post ops over memory regions and work requests and completions

pub trait RdmaWorkRequest {
    type WC: RdmaWorkCompletion;
    type RdmaError: Error;
    type PollError: Error;

    fn poll(&mut self)
    -> Result<RdmaWorkRequestStatus<Self::WC, Self::RdmaError>, Self::PollError>;
}

#[derive(Debug, Clone)]
pub enum RdmaWorkRequestStatus<WC, E> {
    Pending,
    Success(WC),
    Error(E),
}

impl<WC, E> RdmaWorkRequestStatus<WC, E> {
    pub fn pending(&self) -> bool {
        matches!(self, RdmaWorkRequestStatus::Pending)
    }

    pub fn complete(&self) -> bool {
        !self.pending()
    }
}

pub trait RdmaWorkCompletion {
    fn local_modified_len(&self) -> usize;
    fn immediate_data(&self) -> Option<u32>;
}
