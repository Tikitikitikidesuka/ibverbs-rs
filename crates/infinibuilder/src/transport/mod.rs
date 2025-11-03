pub mod basic;
//pub mod retry;
pub mod synced;

use crate::rdma_connection::{
    RdmaConnection, RdmaImmediateDataReceiveConnection, RdmaImmediateDataSendConnection,
    RdmaNamedMemoryRegionConnection, RdmaReadConnection, RdmaReceiveConnection, RdmaSendConnection,
    RdmaWorkCompletion, RdmaWorkRequest, RdmaWriteConnection,
};
use std::error::Error;
use std::ops::RangeBounds;

pub trait RdmaNetworkNodeTransport<Connection: RdmaConnection>:
    RdmaNetworkNodeSendTransport<Connection>
    + RdmaNetworkNodeReceiveTransport<Connection>
    + RdmaNetworkNodeReadTransport<Connection>
    + RdmaNetworkNodeWriteTransport<Connection>
    + RdmaNetworkNodeImmediateDataSendTransport<Connection>
    + RdmaNetworkNodeImmediateDataReceiveTransport<Connection>
{
}

// Blanket implementation
impl<Connection: RdmaConnection, Transport> RdmaNetworkNodeTransport<Connection> for Transport where
    Transport: RdmaNetworkNodeSendTransport<Connection>
        + RdmaNetworkNodeReceiveTransport<Connection>
        + RdmaNetworkNodeReadTransport<Connection>
        + RdmaNetworkNodeWriteTransport<Connection>
        + RdmaNetworkNodeImmediateDataSendTransport<Connection>
        + RdmaNetworkNodeImmediateDataReceiveTransport<Connection>
{
}

pub trait RdmaNetworkNodeSendTransport<Connection: RdmaSendConnection> {
    type WR: RdmaWorkRequest;
    type PostError: Error;

    fn post_send(
        &mut self,
        conn: &mut Connection,
        memory_region: &Connection::MemoryRegion,
        memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<Self::WR, Self::PostError>;
}

pub trait RdmaNetworkNodeReceiveTransport<Connection: RdmaReceiveConnection> {
    type WR: RdmaWorkRequest;
    type PostError: Error;

    fn post_receive(
        &mut self,
        conn: &mut Connection,
        memory_region: &Connection::MemoryRegion,
        memory_range: impl RangeBounds<usize>,
    ) -> Result<Self::WR, Self::PostError>;
}

pub trait RdmaNetworkNodeWriteTransport<Connection: RdmaWriteConnection> {
    type WR: RdmaWorkRequest;
    type PostError: Error;

    fn post_write(
        &mut self,
        conn: &mut Connection,
        local_memory_region: &Connection::MemoryRegion,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: &Connection::RemoteMemoryRegion,
        remote_memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<Self::WR, Self::PostError>;
}

pub trait RdmaNetworkNodeReadTransport<Connection: RdmaReadConnection> {
    type WR: RdmaWorkRequest;
    type PostError: Error;

    fn post_read(
        &mut self,
        conn: &mut Connection,
        local_memory_region: &Connection::MemoryRegion,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: &Connection::RemoteMemoryRegion,
        remote_memory_range: impl RangeBounds<usize>,
    ) -> Result<Self::WR, Self::PostError>;
}

pub trait RdmaNetworkNodeImmediateDataSendTransport<Connection: RdmaImmediateDataSendConnection> {
    type WR: RdmaWorkRequest;
    type PostError: Error;

    fn post_send_immediate_data(
        &mut self,
        conn: &mut Connection,
        immediate_data: u32,
    ) -> Result<Self::WR, Self::PostError>;
}

pub trait RdmaNetworkNodeImmediateDataReceiveTransport<
    Connection: RdmaImmediateDataReceiveConnection,
>
{
    type WR: RdmaWorkRequest;
    type PostError: Error;

    fn post_receive_immediate_data(
        &mut self,
        conn: &mut Connection,
    ) -> Result<Self::WR, Self::PostError>;
}
