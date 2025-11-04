pub mod basic;
//pub mod retry;
//pub mod synced;

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

pub trait RdmaNetworkNodeSendTransport<Connection: RdmaPostSendConnection>:
{
    fn post_send(
        &mut self,
        conn: &mut Connection,
        memory_region: &Connection::MemoryRegion,
        memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<Connection::WorkRequest, Connection::PostError>;
}

pub trait RdmaNetworkNodeReceiveTransport<Connection: RdmaPostReceiveConnection>:
{
    fn post_receive(
        &mut self,
        conn: &mut Connection,
        memory_region: &Connection::MemoryRegion,
        memory_range: impl RangeBounds<usize>,
    ) -> Result<Connection::WorkRequest, Connection::PostError>;
}

pub trait RdmaNetworkNodeWriteTransport<Connection: RdmaPostWriteConnection>:
{
    fn post_write(
        &mut self,
        conn: &mut Connection,
        local_memory_region: &Connection::MemoryRegion,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: &Connection::RemoteMemoryRegion,
        remote_memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<Connection::WorkRequest, Connection::PostError>;
}

pub trait RdmaNetworkNodeReadTransport<Connection: RdmaPostReadConnection>:
{
    fn post_read(
        &mut self,
        conn: &mut Connection,
        local_memory_region: &Connection::MemoryRegion,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: &Connection::RemoteMemoryRegion,
        remote_memory_range: impl RangeBounds<usize>,
    ) -> Result<Connection::WorkRequest, Connection::PostError>;
}

pub trait RdmaNetworkNodeSendImmediateDataTransport<Connection: RdmaPostSendImmediateDataConnection>:
{
    fn post_send_immediate_data(
        &mut self,
        conn: &mut Connection,
        immediate_data: u32,
    ) -> Result<Connection::WorkRequest, Connection::PostError>;
}

pub trait RdmaNetworkNodeReceiveImmediateDataTransport<
    Connection: RdmaPostReceiveImmediateDataConnection,
>
{
    fn post_receive_immediate_data(
        &mut self,
        conn: &mut Connection,
    ) -> Result<Connection::WorkRequest, Connection::PostError>;
}
