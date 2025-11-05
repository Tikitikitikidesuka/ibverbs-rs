use crate::spin_poll::{spin_poll_timeout, spin_poll_timeout_batched};
use std::error::Error;
use std::ops::RangeBounds;
use std::time::Duration;
use thiserror::Error;

pub trait RdmaConnection:
    RdmaMemoryRegionConnection
    + RdmaRemoteMemoryRegionConnection
    + RdmaNamedMemoryRegionConnection
    + RdmaNamedRemoteMemoryRegionConnection
    + RdmaPostSendConnection
    + RdmaPostReceiveConnection
    + RdmaPostReadConnection
    + RdmaPostWriteConnection
    + RdmaPostSendImmediateDataConnection
    + RdmaPostReceiveImmediateDataConnection
{
}

// Blanket implementation
impl<Connection> RdmaConnection for Connection where
    Connection: RdmaMemoryRegionConnection
        + RdmaRemoteMemoryRegionConnection
        + RdmaNamedMemoryRegionConnection
        + RdmaNamedRemoteMemoryRegionConnection
        + RdmaPostSendConnection
        + RdmaPostReceiveConnection
        + RdmaPostReadConnection
        + RdmaPostWriteConnection
        + RdmaPostSendImmediateDataConnection
        + RdmaPostReceiveImmediateDataConnection
{
}

pub trait RdmaMemoryRegionConnection {
    type MemoryRegion;
}

pub trait RdmaRemoteMemoryRegionConnection {
    type RemoteMemoryRegion;
}

pub trait RdmaNamedMemoryRegionConnection: RdmaMemoryRegionConnection {
    fn local_mr(&self, id: impl AsRef<str>) -> Option<Self::MemoryRegion>;
}

pub trait RdmaNamedRemoteMemoryRegionConnection: RdmaRemoteMemoryRegionConnection {
    fn remote_mr(&self, id: impl AsRef<str>) -> Option<Self::RemoteMemoryRegion>;
}

pub trait RdmaPostSendConnection: RdmaMemoryRegionConnection {
    type SendWorkRequest: RdmaWorkRequest;
    type SendPostError: Error;

    fn post_send(
        &mut self,
        memory_region: &Self::MemoryRegion,
        memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<Self::SendWorkRequest, Self::SendPostError>;
}

pub trait RdmaPostReceiveConnection:
    RdmaMemoryRegionConnection
{
    type ReceiveWorkRequest: RdmaWorkRequest;
    type ReceivePostError: Error;

    fn post_receive(
        &mut self,
        memory_region: &Self::MemoryRegion,
        memory_range: impl RangeBounds<usize>,
    ) -> Result<Self::ReceiveWorkRequest, Self::ReceivePostError>;
}

pub trait RdmaPostReadConnection:
    RdmaMemoryRegionConnection + RdmaRemoteMemoryRegionConnection
{
    type ReadWorkRequest: RdmaWorkRequest;
    type ReadPostError: Error;

    fn post_read(
        &mut self,
        local_memory_region: &Self::MemoryRegion,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: &Self::RemoteMemoryRegion,
        remote_memory_range: impl RangeBounds<usize>,
    ) -> Result<Self::ReadWorkRequest, Self::ReadPostError>;
}

pub trait RdmaPostWriteConnection:
    RdmaMemoryRegionConnection + RdmaRemoteMemoryRegionConnection
{
    type WriteWorkRequest: RdmaWorkRequest;
    type WritePostError: Error;

    fn post_write(
        &mut self,
        local_memory_region: &Self::MemoryRegion,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: &Self::RemoteMemoryRegion,
        remote_memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<Self::WriteWorkRequest, Self::WritePostError>;
}

pub trait RdmaPostSendImmediateDataConnection {
    type SendImmediateDataWorkRequest: RdmaWorkRequest;
    type SendImmediateDataPostError: Error;

    fn post_send_immediate_data(
        &mut self,
        immediate_data: u32,
    ) -> Result<Self::SendImmediateDataWorkRequest, Self::SendImmediateDataPostError>;
}

pub trait RdmaPostReceiveImmediateDataConnection {
    type ReceiveImmediateDataWorkRequest: RdmaWorkRequest;
    type ReceiveImmediateDataPostError: Error;

    fn post_receive_immediate_data(&mut self) -> Result<Self::ReceiveImmediateDataWorkRequest, Self::ReceiveImmediateDataPostError>;
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
        spin_poll_timeout(
            || match self.poll() {
                RdmaWorkRequestStatus::Pending => Err(None),
                RdmaWorkRequestStatus::Success(wc) => Ok(wc),
                RdmaWorkRequestStatus::Error(err) => Err(Some(err)),
            },
            timeout,
        )
        .map_err(|error| match error {
            None => WorkRequestSpinPollError::Timeout,
            Some(error) => WorkRequestSpinPollError::ExecutionError(error),
        })
    }

    fn spin_poll_batched(
        &mut self,
        timeout: Duration,
        batch_iters: usize,
    ) -> Result<(Self::WC, Duration), WorkRequestSpinPollError<Self::PollError, Self::RdmaError>>
    {
        spin_poll_timeout_batched(
            || match self.poll() {
                RdmaWorkRequestStatus::Pending => Err(None),
                RdmaWorkRequestStatus::Success(wc) => Ok(wc),
                RdmaWorkRequestStatus::Error(err) => Err(Some(err)),
            },
            timeout,
            batch_iters,
        )
        .map_err(|error| match error {
            None => WorkRequestSpinPollError::Timeout,
            Some(error) => WorkRequestSpinPollError::ExecutionError(error),
        })
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
    #[error("Work request spin poll timeout")]
    Timeout,
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
