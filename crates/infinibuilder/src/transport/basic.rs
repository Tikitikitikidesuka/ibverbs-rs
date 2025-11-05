// Tries to send and if no received was issued, fails

use crate::ibverbs::connection::{IbvConnection, IbvMemoryRegion, IbvRemoteMemoryRegion};
use crate::ibverbs::work_request::IbvWorkRequest;
use crate::rdma_connection::{
    RdmaConnection, RdmaPostReadConnection, RdmaPostReceiveConnection,
    RdmaPostReceiveImmediateDataConnection, RdmaPostSendConnection,
    RdmaPostSendImmediateDataConnection, RdmaPostWriteConnection,
};
use crate::rdma_network_node::{MemoryRegionPair, RdmaNetworkMemoryRegionComponent};
use crate::transport::{
    RdmaNetworkNodeReadTransport, RdmaNetworkNodeReceiveImmediateDataTransport,
    RdmaNetworkNodeReceiveTransport, RdmaNetworkNodeSendImmediateDataTransport,
    RdmaNetworkNodeSendTransport, RdmaNetworkNodeWriteTransport,
};
use std::marker::PhantomData;
use std::ops::RangeBounds;

#[derive(Debug)]
pub struct BasicTransport<Connection> {
    phantom: PhantomData<Connection>,
}

impl<Connection> BasicTransport<Connection> {
    pub fn new() -> Self {
        Self {
            phantom: Default::default(),
        }
    }
}

// Does not register any mr
impl<Connection: RdmaConnection>
    RdmaNetworkMemoryRegionComponent<Connection::MemoryRegion, Connection::RemoteMemoryRegion>
    for BasicTransport<Connection>
{
    type Registered = BasicTransport<Connection>;
    type RegisterError = std::io::Error;

    fn memory(&mut self, _num_connections: usize) -> Option<Vec<(*mut u8, usize)>> {
        None
    }

    fn registered_mrs(
        self,
        _mrs: Option<
            Vec<MemoryRegionPair<Connection::MemoryRegion, Connection::RemoteMemoryRegion>>,
        >,
    ) -> Result<Self::Registered, Self::RegisterError> {
        Ok(self)
    }
}

impl<Connection: RdmaPostSendConnection> RdmaNetworkNodeSendTransport<Connection>
    for BasicTransport<Connection>
{
    type SendTransportWorkRequest = Connection::SendWorkRequest;
    type SendTransportPostError = Connection::SendPostError;

    fn post_send(
        &mut self,
        rank_id: usize,
        conn: &mut Connection,
        memory_region: &Connection::MemoryRegion,
        memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<Connection::SendWorkRequest, Connection::SendPostError> {
        conn.post_send(memory_region, memory_range, immediate_data)
    }
}

impl<Connection: RdmaPostReceiveConnection> RdmaNetworkNodeReceiveTransport<Connection>
    for BasicTransport<Connection>
{
    type ReceiveTransportWorkRequest = Connection::ReceiveWorkRequest;
    type ReceiveTransportPostError = Connection::ReceivePostError;

    fn post_receive(
        &mut self,
        rank_id: usize,
        conn: &mut Connection,
        memory_region: &Connection::MemoryRegion,
        memory_range: impl RangeBounds<usize>,
    ) -> Result<Connection::ReceiveWorkRequest, Connection::ReceivePostError> {
        conn.post_receive(memory_region, memory_range)
    }
}

impl<Connection: RdmaPostWriteConnection> RdmaNetworkNodeWriteTransport<Connection>
    for BasicTransport<Connection>
{
    type WriteTransportWorkRequest = Connection::WriteWorkRequest;
    type WriteTransportPostError = Connection::WritePostError;

    fn post_write(
        &mut self,
        rank_id: usize,
        conn: &mut Connection,
        local_memory_region: &Connection::MemoryRegion,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: &Connection::RemoteMemoryRegion,
        remote_memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<Connection::WriteWorkRequest, Connection::WritePostError> {
        conn.post_write(
            local_memory_region,
            local_memory_range,
            remote_memory_region,
            remote_memory_range,
            immediate_data,
        )
    }
}

impl<Connection: RdmaPostReadConnection> RdmaNetworkNodeReadTransport<Connection>
    for BasicTransport<Connection>
{
    type ReadTransportWorkRequest = Connection::ReadWorkRequest;
    type ReadTransportPostError = Connection::ReadPostError;

    fn post_read(
        &mut self,
        rank_id: usize,
        conn: &mut Connection,
        local_memory_region: &Connection::MemoryRegion,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: &Connection::RemoteMemoryRegion,
        remote_memory_range: impl RangeBounds<usize>,
    ) -> Result<Connection::ReadWorkRequest, Connection::ReadPostError> {
        conn.post_read(
            local_memory_region,
            local_memory_range,
            remote_memory_region,
            remote_memory_range,
        )
    }
}

impl<Connection: RdmaPostSendImmediateDataConnection>
    RdmaNetworkNodeSendImmediateDataTransport<Connection> for BasicTransport<Connection>
{
    type SendImmediateDataTransportWorkRequest = Connection::SendImmediateDataWorkRequest;
    type SendImmediateDataTransportPostError = Connection::SendImmediateDataPostError;

    fn post_send_immediate_data(
        &mut self,
        rank_id: usize,
        conn: &mut Connection,
        immediate_data: u32,
    ) -> Result<Connection::SendImmediateDataWorkRequest, Connection::SendImmediateDataPostError>
    {
        conn.post_send_immediate_data(immediate_data)
    }
}

impl<Connection: RdmaPostReceiveImmediateDataConnection>
    RdmaNetworkNodeReceiveImmediateDataTransport<Connection> for BasicTransport<Connection>
{
    type ReceiveImmediateDataTransportWorkRequest = Connection::ReceiveImmediateDataWorkRequest;
    type ReceiveImmediateDataTransportPostError = Connection::ReceiveImmediateDataPostError;

    fn post_receive_immediate_data(
        &mut self,
        rank_id: usize,
        conn: &mut Connection,
    ) -> Result<
        Connection::ReceiveImmediateDataWorkRequest,
        Connection::ReceiveImmediateDataPostError,
    > {
        conn.post_receive_immediate_data()
    }
}
