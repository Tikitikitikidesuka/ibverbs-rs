pub mod regular;
pub mod retry;
pub mod synced;
pub mod unimplemented;

use crate::rdma_connection::{
    RdmaConnection, RdmaImmediateDataConnection, RdmaReadWriteConnection,
    RdmaSendReceiveConnection, RdmaWorkRequest,
};
use std::error::Error;
use std::ops::RangeBounds;
use std::time::Duration;

pub trait RdmaNetworkTransport<ConnMR, ConnRMR, Conn: RdmaConnection<ConnMR, ConnRMR>>:
    RdmaNetworkSendReceiveTransport<ConnMR, Conn>
    + RdmaNetworkReadWriteTransport<ConnMR, ConnRMR, Conn>
    + RdmaNetworkImmediateDataTransport<Conn>
{
}

// Blanket implementation
impl<ConnMR, ConnRMR, Conn, T> RdmaNetworkTransport<ConnMR, ConnRMR, Conn> for T
where
    Conn: RdmaConnection<ConnMR, ConnRMR>,
    T: RdmaNetworkSendReceiveTransport<ConnMR, Conn>
        + RdmaNetworkReadWriteTransport<ConnMR, ConnRMR, Conn>
        + RdmaNetworkImmediateDataTransport<Conn>,
{
}

pub trait RdmaNetworkSendReceiveTransport<ConnMR, Conn: RdmaSendReceiveConnection<ConnMR>> {
    type WR: RdmaWorkRequest;
    type PostError: Error;

    fn post_send(
        &mut self,
        conn: &mut Conn,
        memory_region: &ConnMR,
        memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<Self::WR, Self::PostError>;

    fn post_receive(
        &mut self,
        conn: &mut Conn,
        memory_region: &ConnMR,
        memory_range: impl RangeBounds<usize>,
    ) -> Result<Self::WR, Self::PostError>;
}

pub trait RdmaNetworkReadWriteTransport<
    ConnMR,
    ConnRMR,
    Conn: RdmaReadWriteConnection<ConnMR, ConnRMR>,
>
{
    type WR: RdmaWorkRequest;
    type PostError: Error;

    fn post_write(
        &mut self,
        conn: &mut Conn,
        local_memory_region: &ConnMR,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: &ConnRMR,
        remote_memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<Self::WR, Self::PostError>;

    fn post_read(
        &mut self,
        conn: &mut Conn,
        local_memory_region: &ConnMR,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: &ConnRMR,
        remote_memory_range: impl RangeBounds<usize>,
    ) -> Result<Self::WR, Self::PostError>;
}

pub trait RdmaNetworkImmediateDataTransport<Conn: RdmaImmediateDataConnection> {
    type WR: RdmaWorkRequest;
    type PostError: Error;

    fn post_send_immediate_data(
        &mut self,
        conn: &mut Conn,
        immediate_data: u32,
    ) -> Result<Self::WR, Self::PostError>;

    fn post_receive_immediate_data(&mut self, conn: &mut Conn)
    -> Result<Self::WR, Self::PostError>;
}
