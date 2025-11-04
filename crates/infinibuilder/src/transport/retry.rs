// Retries until receive is issued or timeout reached...

// Should be done in by defining a custom Work Request that retries...

/*
use crate::barrier::{MemoryRegionPair, RdmaNetworkMemoryRegionComponent};
use crate::rdma_connection::{RdmaImmediateDataConnection, RdmaReadWriteConnection, RdmaSendReceiveConnection, RdmaWorkCompletion, RdmaWorkCompletion};
use crate::spin_poll::spin_poll_timeout_batched;
use crate::transport::{
    RdmaNetworkImmediateDataTransport, RdmaNetworkReadWriteTransport,
    RdmaNetworkSendReceiveTransport,
};
use std::error::Error;
use std::marker::PhantomData;
use std::ops::RangeBounds;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RetryTransportError<PostError: Error> {
    #[error(transparent)]
    PostError(#[from] PostError),
}

#[derive(Debug)]
pub struct RetryTransport<ConnMR, ConnRMR, WR, PostError> {
    phantom_data: PhantomData<(ConnMR, ConnRMR, WR, PostError)>,
    timeout: Duration,
    batch_iters: usize,
}

impl<ConnMR, ConnRMR, WR, PostError> RetryTransport<ConnMR, ConnRMR, WR, PostError> {
    const DEFAULT_BATCH_SIZE: usize = 1024;

    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            phantom_data: Default::default(),
            timeout,
            batch_iters: Self::DEFAULT_BATCH_SIZE,
        }
    }

    pub fn with_batched_timeout(timeout: Duration, batch_size: usize) -> Self {
        Self {
            phantom_data: Default::default(),
            timeout,
            batch_iters: batch_size,
        }
    }
}

// Does not register any mr
impl<ConnMR, ConnRMR, WR, PostError> RdmaNetworkMemoryRegionComponent<ConnMR, ConnRMR>
    for RetryTransport<ConnMR, ConnRMR, WR, PostError>
{
    type Registered = RetryTransport<ConnMR, ConnRMR, WR, PostError>;
    type RegisterError = std::io::Error;

    fn memory(&mut self, _num_connections: usize) -> Option<Vec<(*mut u8, usize)>> {
        None
    }

    fn registered_mrs(
        self,
        _mrs: Option<Vec<MemoryRegionPair<ConnMR, ConnRMR>>>,
    ) -> Result<Self::Registered, Self::RegisterError> {
        Ok(self)
    }
}

impl<
    ConnMR,
    ConnRMR,
    WC: RdmaWorkCompletion,
    PostError: Error,
    Conn: RdmaSendReceiveConnection<ConnMR, WR =WC, PostError = PostError>,
> RdmaNetworkSendReceiveTransport<ConnMR, Conn> for RetryTransport<ConnMR, ConnRMR, WC, PostError>
{
    type WC = WC;
    type TransferError = RetryTransportError<PostError>;

    fn send(
        &mut self,
        conn: &mut Conn,
        memory_region: &ConnMR,
        memory_range: impl RangeBounds<usize> + Clone,
        immediate_data: Option<u32>,
    ) -> Result<Self::WC, Self::TransferError> {
        Ok(spin_poll_timeout_batched(
            || conn.post_send(memory_region, memory_range.clone(), immediate_data),
            self.timeout,
            self.batch_iters,
        )
        .map(|(wr, _elapsed)| wr)?)
    }

    fn receive(
        &mut self,
        conn: &mut Conn,
        memory_region: &ConnMR,
        memory_range: impl RangeBounds<usize> + Clone,
    ) -> Result<Self::WC, Self::TransferError> {
        Ok(spin_poll_timeout_batched(
            || conn.post_receive(memory_region, memory_range.clone()),
            self.timeout,
            self.batch_iters,
        )
        .map(|(wr, _elapsed)| wr)?)
    }
}

impl<
    ConnMR,
    ConnRMR,
    WC: RdmaWorkCompletion,
    PostError: Error,
    Conn: RdmaReadWriteConnection<ConnMR, ConnRMR, WR =WC, PostError = PostError>,
> RdmaNetworkReadWriteTransport<ConnMR, ConnRMR, Conn>
    for RetryTransport<ConnMR, ConnRMR, WC, PostError>
{
    type WC = WC;
    type TransferError = RetryTransportError<PostError>;

    fn write(
        &mut self,
        conn: &mut Conn,
        local_memory_region: &ConnMR,
        local_memory_range: impl RangeBounds<usize> + Clone,
        remote_memory_region: &ConnRMR,
        remote_memory_range: impl RangeBounds<usize> + Clone,
        immediate_data: Option<u32>,
    ) -> Result<Self::WC, Self::TransferError> {
        Ok(spin_poll_timeout_batched(
            || {
                conn.post_write(
                    local_memory_region,
                    local_memory_range.clone(),
                    remote_memory_region,
                    remote_memory_range.clone(),
                    immediate_data,
                )
            },
            self.timeout,
            self.batch_iters,
        )
        .map(|(wr, _elapsed)| wr)?)
    }

    fn read(
        &mut self,
        conn: &mut Conn,
        local_memory_region: &ConnMR,
        local_memory_range: impl RangeBounds<usize> + Clone,
        remote_memory_region: &ConnRMR,
        remote_memory_range: impl RangeBounds<usize> + Clone,
    ) -> Result<Self::WC, Self::TransferError> {
        Ok(spin_poll_timeout_batched(
            || {
                conn.post_read(
                    local_memory_region,
                    local_memory_range.clone(),
                    remote_memory_region,
                    remote_memory_range.clone(),
                )
            },
            self.timeout,
            self.batch_iters,
        )
        .map(|(wr, _elapsed)| wr)?)
    }
}

impl<
    ConnMR,
    ConnRMR,
    WC: RdmaWorkCompletion,
    PostError: Error,
    Conn: RdmaImmediateDataConnection<WR = WC, PostError = PostError>,
> RdmaNetworkImmediateDataTransport<Conn> for RetryTransport<ConnMR, ConnRMR, WC, PostError>
{
    type WC = WC;
    type TransferError = PostError;

    fn send_immediate_data(
        &mut self,
        conn: &mut Conn,
        immediate_data: u32,
    ) -> Result<Self::WC, Self::TransferError> {
        Ok(spin_poll_timeout_batched(
            || conn.post_send_immediate_data(immediate_data),
            self.timeout,
            self.batch_iters,
        )
        .map(|(wr, _elapsed)| wr)?)
    }

    fn receive_immediate_data(
        &mut self,
        conn: &mut Conn,
    ) -> Result<Self::WC, Self::TransferError> {
        Ok(spin_poll_timeout_batched(
            || conn.post_receive_immediate_data(),
            self.timeout,
            self.batch_iters,
        )
        .map(|(wr, _elapsed)| wr)?)
    }
}
*/