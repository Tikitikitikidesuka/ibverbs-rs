// Tries to send and if no received was issued, fails

use crate::barrier::{MemoryRegionPair, RdmaNetworkBarrier, RdmaNetworkMemoryRegionComponent};
use crate::rdma_connection::{
    RdmaConnection, RdmaImmediateDataConnection, RdmaReadWriteConnection,
    RdmaSendReceiveConnection, RdmaWorkRequest,
};
use crate::transport::{
    RdmaNetworkImmediateDataTransport, RdmaNetworkReadWriteTransport,
    RdmaNetworkSendReceiveTransport, RdmaNetworkTransport,
};
use std::error::Error;
use std::marker::PhantomData;
use std::ops::RangeBounds;

#[derive(Debug)]
pub struct UnimplementedTransport<ConnMR, ConnRMR, WR, PostError> {
    phantom_data: PhantomData<(ConnMR, ConnRMR, WR, PostError)>,
}

impl<ConnMR, ConnRMR, WR, PostError> UnimplementedTransport<ConnMR, ConnRMR, WR, PostError> {
    pub fn new() -> Self {
        Self {
            phantom_data: Default::default(),
        }
    }
}

// Does not register any mr
impl<ConnMR, ConnRMR, WR, PostError> RdmaNetworkMemoryRegionComponent<ConnMR, ConnRMR>
    for UnimplementedTransport<ConnMR, ConnRMR, WR, PostError>
{
    type Registered = UnimplementedTransport<ConnMR, ConnRMR, WR, PostError>;
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
> RdmaNetworkSendReceiveTransport<ConnMR, Conn>
    for UnimplementedTransport<ConnMR, ConnRMR, WR, PostError>
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
        todo!()
    }

    fn post_receive(
        &mut self,
        conn: &mut Conn,
        memory_region: &ConnMR,
        memory_range: impl RangeBounds<usize>,
    ) -> Result<Self::WR, Self::PostError> {
        todo!()
    }
}

impl<
    ConnMR,
    ConnRMR,
    WR: RdmaWorkRequest,
    PostError: Error,
    Conn: RdmaReadWriteConnection<ConnMR, ConnRMR, WR = WR, PostError = PostError>,
> RdmaNetworkReadWriteTransport<ConnMR, ConnRMR, Conn>
    for UnimplementedTransport<ConnMR, ConnRMR, WR, PostError>
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
        todo!()
    }

    fn post_read(
        &mut self,
        conn: &mut Conn,
        local_memory_region: &ConnMR,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: &ConnRMR,
        remote_memory_range: impl RangeBounds<usize>,
    ) -> Result<Self::WR, Self::PostError> {
        todo!()
    }
}

impl<
    ConnMR,
    ConnRMR,
    WR: RdmaWorkRequest,
    PostError: Error,
    Conn: RdmaImmediateDataConnection<WR = WR, PostError = PostError>,
> RdmaNetworkImmediateDataTransport<Conn>
    for UnimplementedTransport<ConnMR, ConnRMR, WR, PostError>
{
    type WR = WR;
    type PostError = PostError;

    fn post_send_immediate_data(
        &mut self,
        conn: &mut Conn,
        immediate_data: u32,
    ) -> Result<Self::WR, Self::PostError> {
        todo!()
    }

    fn post_receive_immediate_data(
        &mut self,
        conn: &mut Conn,
    ) -> Result<Self::WR, Self::PostError> {
        todo!()
    }
}
