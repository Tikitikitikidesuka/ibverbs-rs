pub mod any;
pub mod basic;
pub mod synced;

use crate::rdma_connection::{
    RdmaConnection, RdmaPostReadConnection, RdmaPostReceiveConnection,
    RdmaPostReceiveImmediateDataConnection, RdmaPostSendConnection,
    RdmaPostSendImmediateDataConnection, RdmaPostWriteConnection, RdmaWorkCompletion,
    RdmaWorkRequest,
};
use std::borrow::Borrow;
use std::error::Error;
use std::ops::RangeBounds;
use crate::rdma_network_node::{RdmaReadParams, RdmaReceiveParams, RdmaSendParams, RdmaWriteParams};

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

    fn post_send_batch<Range: RangeBounds<usize> + Clone>(
        &mut self,
        rank_id: usize,
        conn: &mut Connection,
        send_params_iter: impl IntoIterator<
            Item = impl Borrow<RdmaSendParams<Connection::MemoryRegion, Range>>,
        >,
    ) -> Vec<Result<Self::WorkRequest, Self::PostError>> {
        send_params_iter
            .into_iter()
            .map(|send_params| {
                self.post_send(
                    rank_id,
                    conn,
                    &send_params.borrow().memory_region,
                    send_params.borrow().memory_range.clone(),
                    send_params.borrow().immediate_data.clone(),
                )
            })
            .collect()
    }
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

    fn post_receive_batch<Range: RangeBounds<usize> + Clone>(
        &mut self,
        rank_id: usize,
        conn: &mut Connection,
        receive_params_iter: impl IntoIterator<
            Item = impl Borrow<RdmaReceiveParams<Connection::MemoryRegion, Range>>,
        >,
    ) -> Vec<Result<Self::WorkRequest, Self::PostError>> {
        receive_params_iter
            .into_iter()
            .map(|receive_params| {
                self.post_receive(
                    rank_id,
                    conn,
                    &receive_params.borrow().memory_region,
                    receive_params.borrow().memory_range.clone(),
                )
            })
            .collect()
    }
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

    fn post_write_batch<Range: RangeBounds<usize> + Clone>(
        &mut self,
        rank_id: usize,
        conn: &mut Connection,
        write_params_iter: impl IntoIterator<
            Item = impl Borrow<
                RdmaWriteParams<Connection::MemoryRegion, Connection::RemoteMemoryRegion, Range>,
            >,
        >,
    ) -> Vec<Result<Self::WorkRequest, Self::PostError>> {
        write_params_iter
            .into_iter()
            .map(|write_params| {
                self.post_write(
                    rank_id,
                    conn,
                    &write_params.borrow().local_memory_region,
                    write_params.borrow().local_memory_range.clone(),
                    &write_params.borrow().remote_memory_region,
                    write_params.borrow().remote_memory_range.clone(),
                    write_params.borrow().immediate_data.clone(),
                )
            })
            .collect()
    }
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

    fn post_read_batch<Range: RangeBounds<usize> + Clone>(
        &mut self,
        rank_id: usize,
        conn: &mut Connection,
        read_params_iter: impl IntoIterator<
            Item = impl Borrow<
                RdmaReadParams<Connection::MemoryRegion, Connection::RemoteMemoryRegion, Range>,
            >,
        >,
    ) -> Vec<Result<Self::WorkRequest, Self::PostError>> {
        read_params_iter
            .into_iter()
            .map(|read_params| {
                self.post_read(
                    rank_id,
                    conn,
                    &read_params.borrow().local_memory_region,
                    read_params.borrow().local_memory_range.clone(),
                    &read_params.borrow().remote_memory_region,
                    read_params.borrow().remote_memory_range.clone(),
                )
            })
            .collect()
    }
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
    ) -> Result<Self::WorkRequest, Self::PostError>;

    fn post_send_immediate_data_batch<Range: RangeBounds<usize> + Clone>(
        &mut self,
        rank_id: usize,
        conn: &mut Connection,
        send_immediate_data_params_iter: impl IntoIterator<Item = u32>,
    ) -> Vec<Result<Self::WorkRequest, Self::PostError>> {
        send_immediate_data_params_iter
            .into_iter()
            .map(|send_immediate_data_params| {
                self.post_send_immediate_data(rank_id, conn, send_immediate_data_params)
            })
            .collect()
    }
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
    ) -> Result<Self::WorkRequest, Self::PostError>;

    fn post_receive_immediate_data_batch<Range: RangeBounds<usize> + Clone>(
        &mut self,
        rank_id: usize,
        conn: &mut Connection,
        num_receives: usize,
    ) -> Vec<Result<Self::WorkRequest, Self::PostError>> {
        (0..num_receives)
            .into_iter()
            .map(|_| self.post_receive_immediate_data(rank_id, conn))
            .collect()
    }
}
