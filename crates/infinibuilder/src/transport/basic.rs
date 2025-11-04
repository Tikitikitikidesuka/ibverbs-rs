// Tries to send and if no received was issued, fails

use crate::ibverbs::connection::{IbvConnection, IbvMemoryRegion, IbvRemoteMemoryRegion};
use crate::ibverbs::work_request::IbvWorkRequest;
use crate::rdma_connection::{
    RdmaPostReadConnection, RdmaPostReceiveConnection, RdmaPostReceiveImmediateDataConnection,
    RdmaPostSendConnection, RdmaPostSendImmediateDataConnection, RdmaPostWriteConnection,
};
use crate::rdma_network_node::{MemoryRegionPair, RdmaNetworkMemoryRegionComponent};
use crate::transport::{
    RdmaNetworkNodeReadTransport, RdmaNetworkNodeReceiveImmediateDataTransport,
    RdmaNetworkNodeReceiveTransport, RdmaNetworkNodeSendImmediateDataTransport,
    RdmaNetworkNodeSendTransport, RdmaNetworkNodeWriteTransport,
};
use std::ops::RangeBounds;

#[derive(Debug)]
pub struct BasicTransport {}

impl BasicTransport {
    pub fn new() -> Self {
        Self {}
    }
}

// Does not register any mr
impl RdmaNetworkMemoryRegionComponent<IbvMemoryRegion, IbvRemoteMemoryRegion> for BasicTransport {
    type Registered = BasicTransport;
    type RegisterError = std::io::Error;

    fn memory(&mut self, _num_connections: usize) -> Option<Vec<(*mut u8, usize)>> {
        None
    }

    fn registered_mrs(
        self,
        _mrs: Option<Vec<MemoryRegionPair<IbvMemoryRegion, IbvRemoteMemoryRegion>>>,
    ) -> Result<Self::Registered, Self::RegisterError> {
        Ok(self)
    }
}

impl RdmaNetworkNodeSendTransport<IbvConnection> for BasicTransport {
    fn post_send(
        &mut self,
        conn: &mut IbvConnection,
        memory_region: &IbvMemoryRegion,
        memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<IbvWorkRequest, std::io::Error> {
        conn.post_send(memory_region, memory_range, immediate_data)
    }
}

impl RdmaNetworkNodeReceiveTransport<IbvConnection> for BasicTransport {
    fn post_receive(
        &mut self,
        conn: &mut IbvConnection,
        memory_region: &IbvMemoryRegion,
        memory_range: impl RangeBounds<usize>,
    ) -> Result<IbvWorkRequest, std::io::Error> {
        conn.post_receive(memory_region, memory_range)
    }
}

impl RdmaNetworkNodeWriteTransport<IbvConnection> for BasicTransport {
    fn post_write(
        &mut self,
        conn: &mut IbvConnection,
        local_memory_region: &IbvMemoryRegion,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: &IbvRemoteMemoryRegion,
        remote_memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<IbvWorkRequest, std::io::Error> {
        conn.post_write(
            local_memory_region,
            local_memory_range,
            remote_memory_region,
            remote_memory_range,
            immediate_data,
        )
    }
}

impl RdmaNetworkNodeReadTransport<IbvConnection> for BasicTransport {
    fn post_read(
        &mut self,
        conn: &mut IbvConnection,
        local_memory_region: &IbvMemoryRegion,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: &IbvRemoteMemoryRegion,
        remote_memory_range: impl RangeBounds<usize>,
    ) -> Result<IbvWorkRequest, std::io::Error> {
        conn.post_read(
            local_memory_region,
            local_memory_range,
            remote_memory_region,
            remote_memory_range,
        )
    }
}

impl RdmaNetworkNodeSendImmediateDataTransport<IbvConnection> for BasicTransport {
    fn post_send_immediate_data(
        &mut self,
        conn: &mut IbvConnection,
        immediate_data: u32,
    ) -> Result<IbvWorkRequest, std::io::Error> {
        conn.post_send_immediate_data(immediate_data)
    }
}

impl RdmaNetworkNodeReceiveImmediateDataTransport<IbvConnection> for BasicTransport {
    fn post_receive_immediate_data(
        &mut self,
        conn: &mut IbvConnection,
    ) -> Result<IbvWorkRequest, std::io::Error> {
        conn.post_receive_immediate_data()
    }
}
