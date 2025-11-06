use crate::rdma_connection::{
    RdmaConnection, RdmaPostReadConnection, RdmaPostReceiveConnection,
    RdmaPostReceiveImmediateDataConnection, RdmaPostSendConnection,
    RdmaPostSendImmediateDataConnection, RdmaPostWriteConnection,
};
use crate::rdma_network_node::{
    MemoryRegionPair, NonMatchingMemoryRegionCount, RdmaNetworkMemoryRegionComponent,
};
use crate::transport::{
    RdmaNetworkNodeReadTransport, RdmaNetworkNodeReceiveImmediateDataTransport,
    RdmaNetworkNodeReceiveTransport, RdmaNetworkNodeSendImmediateDataTransport,
    RdmaNetworkNodeSendTransport, RdmaNetworkNodeWriteTransport,
};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::ops::RangeBounds;

use super::basic::BasicTransport;
use super::synced::{SyncedTransport, UnregisteredSyncedTransport};

#[derive(Debug)]
pub enum AnyUnregisteredTransport<Connection: RdmaConnection> {
    Basic(BasicTransport<Connection>),
    Synced(UnregisteredSyncedTransport<Connection>),
}

#[derive(Debug)]
pub enum AnyTransport<Connection: RdmaConnection> {
    Basic(BasicTransport<Connection>),
    Synced(SyncedTransport<Connection>),
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub enum AnyTransportType {
    Basic,
    Synced(std::time::Duration),
}

#[derive(Debug)]
pub enum AnyTransportError {
    Basic(String),
    Synced(String),
}

impl fmt::Display for AnyTransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnyTransportError::Basic(e) => write!(f, "Basic transport error: {}", e),
            AnyTransportError::Synced(e) => write!(f, "Synced transport error: {}", e),
        }
    }
}

impl Error for AnyTransportError {}

impl<Connection: RdmaConnection>
    RdmaNetworkMemoryRegionComponent<Connection::MemoryRegion, Connection::RemoteMemoryRegion>
    for AnyUnregisteredTransport<Connection>
{
    type Registered = AnyTransport<Connection>;
    type RegisterError = NonMatchingMemoryRegionCount;

    fn memory(&mut self, num_connections: usize) -> Option<Vec<(*mut u8, usize)>> {
        match self {
            AnyUnregisteredTransport::Basic(transport) => transport.memory(num_connections),
            AnyUnregisteredTransport::Synced(transport) => transport.memory(num_connections),
        }
    }

    fn registered_mrs(
        self,
        mrs: Option<
            Vec<MemoryRegionPair<Connection::MemoryRegion, Connection::RemoteMemoryRegion>>,
        >,
    ) -> Result<Self::Registered, Self::RegisterError> {
        match self {
            AnyUnregisteredTransport::Basic(transport) => {
                Ok(AnyTransport::Basic(transport.registered_mrs(mrs)?))
            }
            AnyUnregisteredTransport::Synced(transport) => {
                Ok(AnyTransport::Synced(transport.registered_mrs(mrs)?))
            }
        }
    }
}

impl<Connection: RdmaConnection> AnyTransport<Connection> {
    pub fn new(transport_type: AnyTransportType) -> AnyUnregisteredTransport<Connection> {
        match transport_type {
            AnyTransportType::Basic => AnyUnregisteredTransport::Basic(BasicTransport::new()),
            AnyTransportType::Synced(timeout) => AnyUnregisteredTransport::Synced(
                SyncedTransport::<Connection>::with_post_timeout(timeout),
            ),
        }
    }
}

impl<Connection: RdmaConnection> RdmaNetworkNodeSendTransport<Connection>
    for AnyTransport<Connection>
{
    type WorkRequest = <Connection as RdmaPostSendConnection>::WorkRequest;
    type PostError = AnyTransportError;

    fn post_send(
        &mut self,
        rank_id: usize,
        conn: &mut Connection,
        memory_region: &Connection::MemoryRegion,
        memory_range: impl RangeBounds<usize> + Clone,
        immediate_data: Option<u32>,
    ) -> Result<Self::WorkRequest, Self::PostError> {
        match self {
            AnyTransport::Basic(transport) => transport
                .post_send(rank_id, conn, memory_region, memory_range, immediate_data)
                .map_err(|e| AnyTransportError::Basic(e.to_string())),
            AnyTransport::Synced(transport) => transport
                .post_send(rank_id, conn, memory_region, memory_range, immediate_data)
                .map_err(|e| AnyTransportError::Synced(e.to_string())),
        }
    }
}

impl<Connection: RdmaConnection> RdmaNetworkNodeReceiveTransport<Connection>
    for AnyTransport<Connection>
{
    type WorkRequest = <Connection as RdmaPostReceiveConnection>::WorkRequest;
    type PostError = AnyTransportError;

    fn post_receive(
        &mut self,
        rank_id: usize,
        conn: &mut Connection,
        memory_region: &Connection::MemoryRegion,
        memory_range: impl RangeBounds<usize> + Clone,
    ) -> Result<Self::WorkRequest, Self::PostError> {
        match self {
            AnyTransport::Basic(transport) => transport
                .post_receive(rank_id, conn, memory_region, memory_range)
                .map_err(|e| AnyTransportError::Basic(e.to_string())),
            AnyTransport::Synced(transport) => transport
                .post_receive(rank_id, conn, memory_region, memory_range)
                .map_err(|e| AnyTransportError::Synced(e.to_string())),
        }
    }
}

impl<Connection: RdmaConnection> RdmaNetworkNodeWriteTransport<Connection>
    for AnyTransport<Connection>
{
    type WorkRequest = <Connection as RdmaPostWriteConnection>::WorkRequest;
    type PostError = AnyTransportError;

    fn post_write(
        &mut self,
        rank_id: usize,
        conn: &mut Connection,
        local_memory_region: &Connection::MemoryRegion,
        local_memory_range: impl RangeBounds<usize> + Clone,
        remote_memory_region: &Connection::RemoteMemoryRegion,
        remote_memory_range: impl RangeBounds<usize> + Clone,
        immediate_data: Option<u32>,
    ) -> Result<Self::WorkRequest, Self::PostError> {
        match self {
            AnyTransport::Basic(transport) => transport
                .post_write(
                    rank_id,
                    conn,
                    local_memory_region,
                    local_memory_range,
                    remote_memory_region,
                    remote_memory_range,
                    immediate_data,
                )
                .map_err(|e| AnyTransportError::Basic(e.to_string())),
            AnyTransport::Synced(transport) => transport
                .post_write(
                    rank_id,
                    conn,
                    local_memory_region,
                    local_memory_range,
                    remote_memory_region,
                    remote_memory_range,
                    immediate_data,
                )
                .map_err(|e| AnyTransportError::Synced(e.to_string())),
        }
    }
}

impl<Connection: RdmaConnection> RdmaNetworkNodeReadTransport<Connection>
    for AnyTransport<Connection>
{
    type WorkRequest = <Connection as RdmaPostReadConnection>::WorkRequest;
    type PostError = AnyTransportError;

    fn post_read(
        &mut self,
        rank_id: usize,
        conn: &mut Connection,
        local_memory_region: &Connection::MemoryRegion,
        local_memory_range: impl RangeBounds<usize> + Clone,
        remote_memory_region: &Connection::RemoteMemoryRegion,
        remote_memory_range: impl RangeBounds<usize> + Clone,
    ) -> Result<Self::WorkRequest, Self::PostError> {
        match self {
            AnyTransport::Basic(transport) => transport
                .post_read(
                    rank_id,
                    conn,
                    local_memory_region,
                    local_memory_range,
                    remote_memory_region,
                    remote_memory_range,
                )
                .map_err(|e| AnyTransportError::Basic(e.to_string())),
            AnyTransport::Synced(transport) => transport
                .post_read(
                    rank_id,
                    conn,
                    local_memory_region,
                    local_memory_range,
                    remote_memory_region,
                    remote_memory_range,
                )
                .map_err(|e| AnyTransportError::Synced(e.to_string())),
        }
    }
}

impl<Connection: RdmaConnection> RdmaNetworkNodeSendImmediateDataTransport<Connection>
    for AnyTransport<Connection>
{
    type WorkRequest = <Connection as RdmaPostSendImmediateDataConnection>::WorkRequest;
    type PostError = AnyTransportError;

    fn post_send_immediate_data(
        &mut self,
        rank_id: usize,
        conn: &mut Connection,
        immediate_data: u32,
    ) -> Result<Self::WorkRequest, Self::PostError> {
        match self {
            AnyTransport::Basic(transport) => transport
                .post_send_immediate_data(rank_id, conn, immediate_data)
                .map_err(|e| AnyTransportError::Basic(e.to_string())),
            AnyTransport::Synced(transport) => transport
                .post_send_immediate_data(rank_id, conn, immediate_data)
                .map_err(|e| AnyTransportError::Synced(e.to_string())),
        }
    }
}

impl<Connection: RdmaConnection> RdmaNetworkNodeReceiveImmediateDataTransport<Connection>
    for AnyTransport<Connection>
{
    type WorkRequest = <Connection as RdmaPostReceiveImmediateDataConnection>::WorkRequest;
    type PostError = AnyTransportError;

    fn post_receive_immediate_data(
        &mut self,
        rank_id: usize,
        conn: &mut Connection,
    ) -> Result<Self::WorkRequest, Self::PostError> {
        match self {
            AnyTransport::Basic(transport) => transport
                .post_receive_immediate_data(rank_id, conn)
                .map_err(|e| AnyTransportError::Basic(e.to_string())),
            AnyTransport::Synced(transport) => transport
                .post_receive_immediate_data(rank_id, conn)
                .map_err(|e| AnyTransportError::Synced(e.to_string())),
        }
    }
}
