use std::ops::RangeBounds;

pub trait RdmaConnection {
    type MR;
    type RemoteMR;
    type WR: RdmaWorkRequest;
    type WC: RdmaWorkCompletion;
    type PostError;

    fn post_send(
        &mut self,
        memory_region: Self::MR,
        memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<Self::WR, Self::PostError>;

    fn post_receive(
        &mut self,
        memory_region: Self::MR,
        memory_range: impl RangeBounds<usize>,
    ) -> Result<Self::WR, Self::PostError>;

    fn post_write(
        &mut self,
        local_memory_region: Self::MR,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: Self::RemoteMR,
        remote_memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<Self::WR, Self::PostError>;

    fn post_read(
        &mut self,
        local_memory_region: Self::MR,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: Self::RemoteMR,
        remote_memory_range: impl RangeBounds<usize>,
    ) -> Result<Self::WR, Self::PostError>;
}

// No traits for QP, PD or CQ as those the user should not care about in this abstraction
// Only things user should interact are post ops over memory regions and work requests and completions

pub trait RdmaWorkRequest {
    type WC: RdmaWorkCompletion;
    type PollError;

    fn poll(&mut self) -> RdmaWorkRequestStatus<Self::WC, Self::PollError>;
}

pub enum RdmaWorkRequestStatus<WC, E> {
    Pending,
    Success(WC),
    Error(E),
}

pub trait RdmaWorkCompletion {
    fn local_modified_len(&self) -> usize;
    fn immediate_data(&self) -> Option<u32>;
}
