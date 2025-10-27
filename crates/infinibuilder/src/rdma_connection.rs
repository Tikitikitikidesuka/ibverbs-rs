use crate::ibverbs::work_request::IbvWorkRequest;
use crate::spin_poll::{Timeout, spin_poll, spin_poll_batched};
use std::error::Error;
use std::ops::RangeBounds;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RdmaPostError {
    #[error("Memory region {0:?} not registered")]
    InvalidMemoryRegion(RdmaMemoryRegion),

    #[error("Remote memory region {0:?} not registered")]
    InvalidRemoteMemoryRegion(RdmaRemoteMemoryRegion),

    #[error("Invalid memory range {from}..={to_inclusive} for memory region {mr_idx}")]
    InvalidRange {
        from: usize,
        to_inclusive: usize,
        mr_idx: usize,
    },

    #[error("Transport error: {0}")]
    IoError(#[from] std::io::Error),

    // Implementation-specific details
    #[error("Implementation error")]
    Implementation(#[source] Box<dyn Error + Send + Sync>),
}

/// Wrap Memory Region index to reduce valid mr input space
#[derive(Debug, Copy, Clone)]
pub struct RdmaMemoryRegion {
    pub(super) idx: usize,
}

/// Wrap Remote Memory Region index to reduce valid mr input space
#[derive(Debug, Copy, Clone)]
pub struct RdmaRemoteMemoryRegion {
    pub(super) idx: usize,
}

pub trait RdmaConnection:
    RdmaSendReceiveConnection + RdmaReadWriteConnection + RdmaImmediateDataConnection
{
}

pub trait RdmaSendReceiveConnection {
    type WR: RdmaWorkRequest;

    // Posts a send operation. Will fail if the remote has not posted a receive operation before hand.
    fn post_send(
        &mut self,
        memory_region: RdmaMemoryRegion,
        memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<Self::WR, RdmaPostError>;

    // Posts a receive operation.
    fn post_receive(
        &mut self,
        memory_region: RdmaMemoryRegion,
        memory_range: impl RangeBounds<usize>,
    ) -> Result<Self::WR, RdmaPostError>;
}

pub trait RdmaReadWriteConnection {
    type WR: RdmaWorkRequest;

    // Posts a write operation.
    // If sent with immediate data, the data must be obtained in the remote peer
    // by calling post_receive_immediate
    fn post_write(
        &mut self,
        local_memory_region: RdmaMemoryRegion,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: RdmaRemoteMemoryRegion,
        remote_memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<Self::WR, RdmaPostError>;

    // Posts a read operation.
    fn post_read(
        &mut self,
        local_memory_region: RdmaMemoryRegion,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: RdmaRemoteMemoryRegion,
        remote_memory_range: impl RangeBounds<usize>,
    ) -> Result<Self::WR, RdmaPostError>;
}

pub trait RdmaImmediateDataConnection {
    type WR: RdmaWorkRequest;

    fn post_send_immediate_data(
        &mut self,
        immediate_data: u32,
    ) -> Result<Self::WR, std::io::Error>;

    fn post_receive_immediate_data(&mut self) -> Result<Self::WR, std::io::Error>;
}

// No traits for QP, PD or CQ as those the user should not care about in this abstraction
// Only things user should interact are post ops over memory regions and work requests and completions

pub trait RdmaWorkRequest {
    type WC: RdmaWorkCompletion;
    type RdmaError: Error;
    type PollError: Error;

    fn poll(
        &mut self,
    ) -> RdmaWorkRequestStatus<Self::WC, WorkRequestPollError<Self::PollError, Self::RdmaError>>;

    fn spin_poll(
        &mut self,
        timeout: Duration,
    ) -> Result<(Self::WC, Duration), WorkRequestSpinPollError<Self::PollError, Self::RdmaError>>
    {
        match spin_poll(
            || match self.poll() {
                RdmaWorkRequestStatus::Pending => None,
                RdmaWorkRequestStatus::Success(wc) => Some(Ok(wc)),
                RdmaWorkRequestStatus::Error(err) => Some(Err(err)),
            },
            timeout,
        ) {
            Ok((inner, dur)) => inner
                .map(|wc| (wc, dur))
                .map_err(|error| WorkRequestSpinPollError::ExecutionError(error)),
            Err(timeout) => Err(WorkRequestSpinPollError::Timeout(timeout)),
        }
    }

    fn spin_poll_batched(
        &mut self,
        timeout: Duration,
        batch_iters: usize,
    ) -> Result<(Self::WC, Duration), WorkRequestSpinPollError<Self::PollError, Self::RdmaError>>
    {
        match spin_poll_batched(
            || match self.poll() {
                RdmaWorkRequestStatus::Pending => None,
                RdmaWorkRequestStatus::Success(wc) => Some(Ok(wc)),
                RdmaWorkRequestStatus::Error(err) => Some(Err(err)),
            },
            timeout,
            batch_iters,
        ) {
            Ok((inner, dur)) => inner
                .map(|wc| (wc, dur))
                .map_err(|error| WorkRequestSpinPollError::ExecutionError(error)),
            Err(timeout) => Err(WorkRequestSpinPollError::Timeout(timeout)),
        }
    }
}

#[derive(Debug, Error)]
pub enum WorkRequestPollError<PollError: Error, RdmaError: Error> {
    #[error("Work request poll error: {0}")]
    PollError(PollError),
    #[error("Work request poll rdma error: {0}")]
    RdmaError(RdmaError),
}

#[derive(Debug, Error)]
pub enum WorkRequestSpinPollError<PollError: Error, RdmaError: Error> {
    #[error("Work request spin poll timeout: {0}")]
    Timeout(Timeout),
    #[error("Work request spin poll error: {0}")]
    ExecutionError(#[from] WorkRequestPollError<PollError, RdmaError>),
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
