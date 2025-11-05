pub mod basic;
//pub mod retry;
pub mod synced;

use crate::rdma_connection::{
    RdmaConnection, RdmaPostReadConnection, RdmaPostReceiveConnection,
    RdmaPostReceiveImmediateDataConnection, RdmaPostSendConnection,
    RdmaPostSendImmediateDataConnection, RdmaPostWriteConnection, RdmaWorkCompletion,
    RdmaWorkRequest,
};
use std::error::Error;
use std::ops::RangeBounds;

pub trait RdmaNetworkNodeTransport<Connection: RdmaConnection>:
    RdmaNetworkNodeSendTransport<Connection>
    + RdmaNetworkNodeReceiveTransport<Connection>
    + RdmaNetworkNodeReadTransport<Connection>
    + RdmaNetworkNodeWriteTransport<Connection>
    + RdmaNetworkNodeSendImmediateDataTransport<Connection>
    + RdmaNetworkNodeReceiveImmediateDataTransport<Connection>
{
}

// Blanket implementation
impl<Connection: RdmaConnection, Transport> RdmaNetworkNodeTransport<Connection> for Transport where
    Transport: RdmaNetworkNodeSendTransport<Connection>
        + RdmaNetworkNodeReceiveTransport<Connection>
        + RdmaNetworkNodeReadTransport<Connection>
        + RdmaNetworkNodeWriteTransport<Connection>
        + RdmaNetworkNodeSendImmediateDataTransport<Connection>
        + RdmaNetworkNodeReceiveImmediateDataTransport<Connection>
{
}

pub trait RdmaNetworkNodeSendTransport<Connection: RdmaPostSendConnection> {
    type WorkRequest: RdmaWorkRequest;
    type PostError: Error;

    fn post_send(
        &mut self,
        rank_id: usize,
        conn: &mut Connection,
        memory_region: &Connection::MemoryRegion,
        memory_range: impl RangeBounds<usize> + Clone,
        immediate_data: Option<u32>,
    ) -> Result<Self::WorkRequest, Self::PostError>;
}

pub trait RdmaNetworkNodeReceiveTransport<Connection: RdmaPostReceiveConnection> {
    type WorkRequest: RdmaWorkRequest;
    type PostError: Error;

    fn post_receive(
        &mut self,
        rank_id: usize,
        conn: &mut Connection,
        memory_region: &Connection::MemoryRegion,
        memory_range: impl RangeBounds<usize> + Clone,
    ) -> Result<Self::WorkRequest, Self::PostError>;
}

pub trait RdmaNetworkNodeWriteTransport<Connection: RdmaPostWriteConnection> {
    type WorkRequest: RdmaWorkRequest;
    type PostError: Error;

    fn post_write(
        &mut self,
        rank_id: usize,
        conn: &mut Connection,
        local_memory_region: &Connection::MemoryRegion,
        local_memory_range: impl RangeBounds<usize> + Clone,
        remote_memory_region: &Connection::RemoteMemoryRegion,
        remote_memory_range: impl RangeBounds<usize> + Clone,
        immediate_data: Option<u32>,
    ) -> Result<Self::WorkRequest, Self::PostError>;
}

pub trait RdmaNetworkNodeReadTransport<Connection: RdmaPostReadConnection> {
    type WorkRequest: RdmaWorkRequest;
    type PostError: Error;

    fn post_read(
        &mut self,
        rank_id: usize,
        conn: &mut Connection,
        local_memory_region: &Connection::MemoryRegion,
        local_memory_range: impl RangeBounds<usize> + Clone,
        remote_memory_region: &Connection::RemoteMemoryRegion,
        remote_memory_range: impl RangeBounds<usize> + Clone,
    ) -> Result<Self::WorkRequest, Self::PostError>;
}

pub trait RdmaNetworkNodeSendImmediateDataTransport<Connection: RdmaPostSendImmediateDataConnection>
{
    type WorkRequest: RdmaWorkRequest;
    type PostError: Error;

    fn post_send_immediate_data(
        &mut self,
        rank_id: usize,
        conn: &mut Connection,
        immediate_data: u32,
    ) -> Result<
        Self::WorkRequest,
        Self::PostError,
    >;
}

pub trait RdmaNetworkNodeReceiveImmediateDataTransport<
    Connection: RdmaPostReceiveImmediateDataConnection,
>
{
    type WorkRequest: RdmaWorkRequest;
    type PostError: Error;

    fn post_receive_immediate_data(
        &mut self,
        rank_id: usize,
        conn: &mut Connection,
    ) -> Result<
        Self::WorkRequest,
        Self::PostError,
    >;
}
