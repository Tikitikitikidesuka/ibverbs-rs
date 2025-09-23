use crate::connect::Connect;
use crate::ibverbs::simple_unit::IbvSimpleUnit;
use crate::ibverbs::simple_unit::connection::{IbvConnection, UnconnectedIbvConnection};
use crate::ibverbs::simple_unit::mode::Mode;
use crate::ibverbs::unsafe_slice::UnsafeSlice;
use crate::ibverbs::work_request::CachedWorkRequest;
use crate::rdma_traits::{RdmaReadWrite, RdmaSendRecv, WorkRequest};
use ibverbs::{MemoryRegion, RemoteMemoryRegion};
use serde::{Deserialize, Serialize};
use std::ops::RangeBounds;

pub struct TransferMode<const POLL_BUFF_SIZE: usize>;

impl<const POLL_BUFF_SIZE: usize> Mode for TransferMode<POLL_BUFF_SIZE> {
    type UnconnectedMr = UnconnectedTransferMr<POLL_BUFF_SIZE>;
    type ConnectedMr = ConnectedTransferMr<POLL_BUFF_SIZE>;
    type MrConnectionConfig = TransferMrConnectionConfig;
}

pub struct UnconnectedTransferMr<const POLL_BUFF_SIZE: usize> {
    mr: MemoryRegion<UnsafeSlice<u8>>,
}

impl<const POLL_BUFF_SIZE: usize> UnconnectedTransferMr<POLL_BUFF_SIZE> {
    pub(super) unsafe fn new(
        connection: &mut UnconnectedIbvConnection,
        memory: &[u8],
    ) -> std::io::Result<Self> {
        let mr = connection
            .pd
            .register(unsafe { UnsafeSlice::new(memory) })?;
        Ok(Self { mr })
    }
}

impl<const POLL_BUFF_SIZE: usize> Connect for UnconnectedTransferMr<POLL_BUFF_SIZE> {
    type ConnectionConfig = TransferMrConnectionConfig;
    type Connected = ConnectedTransferMr<POLL_BUFF_SIZE>;

    fn connection_config(&self) -> Self::ConnectionConfig {
        TransferMrConnectionConfig {
            remote_mr: self.mr.remote(),
        }
    }

    fn connect(
        self,
        connection_config: Self::ConnectionConfig,
    ) -> std::io::Result<Self::Connected> {
        Ok(ConnectedTransferMr {
            mr: self.mr,
            remote_mr: connection_config.remote_mr,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransferMrConnectionConfig {
    remote_mr: RemoteMemoryRegion,
}

pub struct ConnectedTransferMr<const POLL_BUFF_SIZE: usize> {
    mr: MemoryRegion<UnsafeSlice<u8>>,
    remote_mr: RemoteMemoryRegion,
}

impl<const POLL_BUFF_SIZE: usize> ConnectedTransferMr<POLL_BUFF_SIZE> {
    pub(super) unsafe fn post_send(
        &self,
        connection: &mut IbvConnection,
        mr_range: impl RangeBounds<usize>,
        imm_data: Option<u32>,
    ) -> std::io::Result<CachedWorkRequest<POLL_BUFF_SIZE>> {
        let wr_id = connection.cached_cq.fetch_advance_next_wr_id();
        unsafe {
            connection
                .qp
                .post_send(&[self.mr.slice(mr_range)], wr_id, imm_data)
        }?;
        Ok(CachedWorkRequest::new(wr_id, connection.cached_cq.clone()))
    }

    pub(super) unsafe fn post_receive(
        &self,
        connection: &mut IbvConnection,
        mr_range: impl RangeBounds<usize>,
    ) -> std::io::Result<CachedWorkRequest<POLL_BUFF_SIZE>> {
        let wr_id = connection.cached_cq.fetch_advance_next_wr_id();
        unsafe {
            connection
                .qp
                .post_receive(&[self.mr.slice(mr_range)], wr_id)
        }?;
        Ok(CachedWorkRequest::new(wr_id, connection.cached_cq.clone()))
    }

    pub(super) unsafe fn post_write(
        &self,
        connection: &mut IbvConnection,
        mr_range: impl RangeBounds<usize>,
        remote_mr_range: impl RangeBounds<usize>,
        imm_data: Option<u32>,
    ) -> std::io::Result<CachedWorkRequest<POLL_BUFF_SIZE>> {
        let wr_id = connection.cached_cq.fetch_advance_next_wr_id();
        connection.qp.post_write(
            &[self.mr.slice(mr_range)],
            self.remote_mr.slice(remote_mr_range),
            wr_id,
            imm_data,
        )?;
        Ok(CachedWorkRequest::new(wr_id, connection.cached_cq.clone()))
    }

    pub(super) unsafe fn post_read(
        &self,
        connection: &mut IbvConnection,
        mr_range: impl RangeBounds<usize>,
        remote_mr_range: impl RangeBounds<usize>,
    ) -> std::io::Result<CachedWorkRequest<POLL_BUFF_SIZE>> {
        let wr_id = connection.cached_cq.fetch_advance_next_wr_id();
        connection.qp.post_read(
            &[self.mr.slice(mr_range)],
            self.remote_mr.slice(remote_mr_range),
            wr_id,
        )?;
        Ok(CachedWorkRequest::new(wr_id, connection.cached_cq.clone()))
    }
}

impl<const POLL_BUFF_SIZE: usize> RdmaSendRecv for IbvSimpleUnit<TransferMode<POLL_BUFF_SIZE>> {
    unsafe fn post_send(
        &mut self,
        mr_range: impl RangeBounds<usize>,
        imm_data: Option<u32>,
    ) -> std::io::Result<impl WorkRequest + 'static> {
        unsafe { self.mr.post_send(&mut self.connection, mr_range, imm_data) }
    }

    unsafe fn post_receive(
        &mut self,
        mr_range: impl RangeBounds<usize>,
    ) -> std::io::Result<impl WorkRequest + 'static> {
        unsafe { self.mr.post_receive(&mut self.connection, mr_range) }
    }
}

impl<const POLL_BUFF_SIZE: usize> RdmaReadWrite for IbvSimpleUnit<TransferMode<POLL_BUFF_SIZE>> {
    unsafe fn post_write(
        &mut self,
        mr_range: impl RangeBounds<usize>,
        remote_mr_range: impl RangeBounds<usize>,
        imm_data: Option<u32>,
    ) -> std::io::Result<impl WorkRequest + 'static> {
        unsafe {
            self.mr
                .post_write(&mut self.connection, mr_range, remote_mr_range, imm_data)
        }
    }

    unsafe fn post_read(
        &mut self,
        mr_range: impl RangeBounds<usize>,
        remote_mr_range: impl RangeBounds<usize>,
    ) -> std::io::Result<impl WorkRequest + 'static> {
        unsafe {
            self.mr
                .post_read(&mut self.connection, mr_range, remote_mr_range)
        }
    }
}
