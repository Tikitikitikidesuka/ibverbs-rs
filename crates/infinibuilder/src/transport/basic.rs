// Tries to send and if no received was issued, fails

use crate::barrier::{MemoryRegionPair, RdmaNetworkBarrier, RdmaNetworkMemoryRegionComponent};
use crate::rdma_connection::{
    RdmaImmediateDataConnection, RdmaReadWriteConnection, RdmaSendReceiveConnection,
    RdmaWorkRequest,
};
use crate::transport::{
    RdmaNetworkImmediateDataTransport, RdmaNetworkReadWriteTransport,
    RdmaNetworkSendReceiveTransport,
};
use std::error::Error;
use std::marker::PhantomData;
use std::ops::RangeBounds;
use std::time::Duration;

#[derive(Debug)]
pub struct Transport<ConnMR, ConnRMR, WR, PostError> {
    phantom_data: PhantomData<(ConnMR, ConnRMR, WR, PostError)>,
}

impl<ConnMR, ConnRMR, WR, PostError> Transport<ConnMR, ConnRMR, WR, PostError> {
    pub fn new() -> Self {
        Self {
            phantom_data: Default::default(),
        }
    }
}

// Does not register any mr
impl<ConnMR, ConnRMR, WR, PostError> RdmaNetworkMemoryRegionComponent<ConnMR, ConnRMR>
    for Transport<ConnMR, ConnRMR, WR, PostError>
{
    type Registered = Transport<ConnMR, ConnRMR, WR, PostError>;
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
    WR: RdmaWorkRequest,
    PostError: Error,
    Conn: RdmaSendReceiveConnection<ConnMR, WR = WR, PostError = PostError>,
> RdmaNetworkSendReceiveTransport<ConnMR, Conn> for Transport<ConnMR, ConnRMR, WR, PostError>
{
    type WR = WR;
    type PostError = PostError;

    fn post_send(
        &mut self,
        conn: &mut Conn,
        memory_region: &ConnMR,
        memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<Self::WR, Self::PostError> {
        conn.post_send(memory_region, memory_range, immediate_data)
    }

    fn post_receive(
        &mut self,
        conn: &mut Conn,
        memory_region: &ConnMR,
        memory_range: impl RangeBounds<usize>,
    ) -> Result<Self::WR, Self::PostError> {
        conn.post_receive(memory_region, memory_range)
    }
}

impl<
    ConnMR,
    ConnRMR,
    WR: RdmaWorkRequest,
    PostError: Error,
    Conn: RdmaReadWriteConnection<ConnMR, ConnRMR, WR = WR, PostError = PostError>,
> RdmaNetworkReadWriteTransport<ConnMR, ConnRMR, Conn>
    for Transport<ConnMR, ConnRMR, WR, PostError>
{
    type WR = WR;
    type PostError = PostError;

    fn post_write(
        &mut self,
        conn: &mut Conn,
        local_memory_region: &ConnMR,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: &ConnRMR,
        remote_memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<Self::WR, Self::PostError> {
        conn.post_write(
            local_memory_region,
            local_memory_range,
            remote_memory_region,
            remote_memory_range,
            immediate_data,
        )
    }

    fn post_read(
        &mut self,
        conn: &mut Conn,
        local_memory_region: &ConnMR,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: &ConnRMR,
        remote_memory_range: impl RangeBounds<usize>,
    ) -> Result<Self::WR, Self::PostError> {
        conn.post_read(
            local_memory_region,
            local_memory_range,
            remote_memory_region,
            remote_memory_range,
        )
    }
}

impl<
    ConnMR,
    ConnRMR,
    WR: RdmaWorkRequest,
    PostError: Error,
    Conn: RdmaImmediateDataConnection<WR = WR, PostError = PostError>,
> RdmaNetworkImmediateDataTransport<Conn> for Transport<ConnMR, ConnRMR, WR, PostError>
{
    type WR = WR;
    type PostError = PostError;

    fn post_send_immediate_data(
        &mut self,
        conn: &mut Conn,
        immediate_data: u32,
    ) -> Result<Self::WR, Self::PostError> {
        conn.post_send_immediate_data(immediate_data)
    }

    fn post_receive_immediate_data(
        &mut self,
        conn: &mut Conn,
    ) -> Result<Self::WR, Self::PostError> {
        conn.post_receive_immediate_data()
    }
}
