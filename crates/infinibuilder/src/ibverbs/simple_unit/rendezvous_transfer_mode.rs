use crate::connect::Connect;
use crate::ibverbs::simple_unit::IbvSimpleUnit;
use crate::ibverbs::simple_unit::connection::UnconnectedIbvConnection;
use crate::ibverbs::simple_unit::mode::Mode;
use crate::ibverbs::simple_unit::rendezvous_mode::{
    ConnectedSyncMr, SyncMrConnectionConfig, UnconnectedSyncMr,
};
use crate::ibverbs::simple_unit::transfer_mode::{
    ConnectedTransferMr, TransferMrConnectionConfig, UnconnectedTransferMr,
};
use crate::rdma_traits::{RdmaReadWrite, RdmaSendRecv, RdmaSync, SyncState, Timeout, WorkRequest};
use serde::{Deserialize, Serialize};
use std::ops::RangeBounds;
use std::time::Duration;

#[derive(Debug, Copy, Clone)]
pub struct SyncTransferMode<const POLL_BUFF_SIZE: usize>;

impl<const POLL_BUFF_SIZE: usize> Mode for SyncTransferMode<POLL_BUFF_SIZE> {
    type UnconnectedMr = UnconnectedSyncTransferMr<POLL_BUFF_SIZE>;
    type ConnectedMr = ConnectedSyncTransferMr<POLL_BUFF_SIZE>;
    type MrConnectionConfig = SyncTransferMrConnectionConfig;
}

pub struct UnconnectedSyncTransferMr<const POLL_BUFF_SIZE: usize> {
    transfer_mr: UnconnectedTransferMr<POLL_BUFF_SIZE>,
    sync_mr: UnconnectedSyncMr,
}

impl<const POLL_BUFF_SIZE: usize> UnconnectedSyncTransferMr<POLL_BUFF_SIZE> {
    pub unsafe fn new(
        connection: &mut UnconnectedIbvConnection,
        memory_ptr: *mut u8,
        memory_length: usize,
    ) -> std::io::Result<Self> {
        Ok(Self {
            transfer_mr: unsafe {
                UnconnectedTransferMr::new(connection, memory_ptr, memory_length)?
            },
            sync_mr: UnconnectedSyncMr::new(connection)?,
        })
    }
}

impl<const POLL_BUFF_SIZE: usize> Connect for UnconnectedSyncTransferMr<POLL_BUFF_SIZE> {
    type ConnectionConfig = SyncTransferMrConnectionConfig;
    type Connected = ConnectedSyncTransferMr<POLL_BUFF_SIZE>;

    fn connection_config(&self) -> Self::ConnectionConfig {
        SyncTransferMrConnectionConfig {
            transfer_mr_connection_config: self.transfer_mr.connection_config(),
            sync_mr_connection_config: self.sync_mr.connection_config(),
        }
    }

    fn connect(
        self,
        connection_config: Self::ConnectionConfig,
    ) -> std::io::Result<Self::Connected> {
        Ok(ConnectedSyncTransferMr {
            transfer_mr: self
                .transfer_mr
                .connect(connection_config.transfer_mr_connection_config)?,
            sync_mr: self
                .sync_mr
                .connect(connection_config.sync_mr_connection_config)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncTransferMrConnectionConfig {
    transfer_mr_connection_config: TransferMrConnectionConfig,
    sync_mr_connection_config: SyncMrConnectionConfig,
}

pub struct ConnectedSyncTransferMr<const POLL_BUFF_SIZE: usize> {
    transfer_mr: ConnectedTransferMr<POLL_BUFF_SIZE>,
    sync_mr: ConnectedSyncMr,
}

impl<const POLL_BUFF_SIZE: usize> RdmaSendRecv for IbvSimpleUnit<SyncTransferMode<POLL_BUFF_SIZE>> {
    unsafe fn post_send(
        &mut self,
        mr_range: impl RangeBounds<usize>,
        imm_data: Option<u32>,
    ) -> std::io::Result<impl WorkRequest + 'static> {
        unsafe {
            self.mr
                .transfer_mr
                .post_send(&mut self.connection, mr_range, imm_data)
        }
    }

    unsafe fn post_receive(
        &mut self,
        mr_range: impl RangeBounds<usize>,
    ) -> std::io::Result<impl WorkRequest + 'static> {
        unsafe {
            self.mr
                .transfer_mr
                .post_receive(&mut self.connection, mr_range)
        }
    }
}

impl<const POLL_BUFF_SIZE: usize> RdmaReadWrite
    for IbvSimpleUnit<SyncTransferMode<POLL_BUFF_SIZE>>
{
    unsafe fn post_write(
        &mut self,
        mr_range: impl RangeBounds<usize>,
        remote_mr_range: impl RangeBounds<usize>,
        imm_data: Option<u32>,
    ) -> std::io::Result<impl WorkRequest + 'static> {
        unsafe {
            self.mr.transfer_mr.post_write(
                &mut self.connection,
                mr_range,
                remote_mr_range,
                imm_data,
            )
        }
    }

    unsafe fn post_read(
        &mut self,
        mr_range: impl RangeBounds<usize>,
        remote_mr_range: impl RangeBounds<usize>,
    ) -> std::io::Result<impl WorkRequest + 'static> {
        unsafe {
            self.mr
                .transfer_mr
                .post_read(&mut self.connection, mr_range, remote_mr_range)
        }
    }
}

impl<const POLL_BUFF_SIZE: usize> RdmaSync for IbvSimpleUnit<SyncTransferMode<POLL_BUFF_SIZE>> {
    fn sync_state(&self) -> SyncState {
        self.mr.sync_mr.sync_state()
    }

    fn signal_peer(&mut self) -> Option<std::io::Result<()>> {
        self.mr
            .sync_mr
            .signal_peer::<POLL_BUFF_SIZE>(&mut self.connection)
    }

    fn synchronize(&mut self) -> std::io::Result<()> {
        self.mr
            .sync_mr
            .synchronize::<POLL_BUFF_SIZE>(&mut self.connection)
    }

    fn synchronize_with_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<std::io::Result<()>, Timeout> {
        self.mr
            .sync_mr
            .synchronize_with_timeout::<POLL_BUFF_SIZE>(&mut self.connection, timeout)
    }
}
