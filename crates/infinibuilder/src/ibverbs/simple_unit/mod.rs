use crate::connect::Connect;
use crate::ibverbs::simple_unit::connection::{
    IbvConnection, IbvConnectionConfig, UnconnectedIbvConnection,
};
use crate::ibverbs::simple_unit::mode::Mode;
use crate::ibverbs::simple_unit::rendezvous_mode::{SyncMode, UnconnectedSyncMr};
use crate::ibverbs::simple_unit::rendezvous_transfer_mode::{
    SyncTransferMode, UnconnectedSyncTransferMr,
};
use crate::ibverbs::simple_unit::transfer_mode::{TransferMode, UnconnectedTransferMr};
use ibverbs::Context;
use serde::{Deserialize, Serialize};
use std::ops::DerefMut;

mod connection;
pub mod mode;
pub mod network;
pub mod rendezvous_mode;
pub mod rendezvous_transfer_mode;
pub mod transfer_mode;
//mod sync_transfer_mode;

pub struct UnconnectedIbvSimpleUnit<M: Mode> {
    connection: UnconnectedIbvConnection,
    mr: M::UnconnectedMr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IbvSimpleUnitConnectionConfig<M: Mode> {
    connection_config: IbvConnectionConfig,
    mr_connection_config: M::MrConnectionConfig,
}

pub struct IbvSimpleUnit<M: Mode> {
    connection: IbvConnection,
    mr: M::ConnectedMr,
}

impl IbvSimpleUnit<TransferMode<0>> {
    pub unsafe fn new_transfer_unit<const CQ_SIZE: usize, const POLL_BUFF_SIZE: usize>(
        ibv_context: &Context,
        memory_ptr: *mut u8,
        memory_length: usize,
    ) -> std::io::Result<UnconnectedIbvSimpleUnit<TransferMode<POLL_BUFF_SIZE>>> {
        let mut connection = UnconnectedIbvConnection::new::<CQ_SIZE>(ibv_context)?;
        let mr = UnconnectedTransferMr::new(&mut connection, memory_ptr, memory_length)?;
        Ok(UnconnectedIbvSimpleUnit { connection, mr })
    }
}

impl IbvSimpleUnit<SyncMode> {
    pub fn new_sync_unit(
        ibv_context: &Context,
    ) -> std::io::Result<UnconnectedIbvSimpleUnit<SyncMode>> {
        let mut connection = UnconnectedIbvConnection::new::<1>(ibv_context)?;
        let mr = UnconnectedSyncMr::new(&mut connection)?;
        Ok(UnconnectedIbvSimpleUnit { connection, mr })
    }
}

impl IbvSimpleUnit<SyncTransferMode<0>> {
    pub unsafe fn new_sync_transfer_unit<const CQ_SIZE: usize, const POLL_BUFF_SIZE: usize>(
        ibv_context: &Context,
        memory_ptr: *mut u8,
        memory_length: usize,
    ) -> std::io::Result<UnconnectedIbvSimpleUnit<SyncTransferMode<POLL_BUFF_SIZE>>> {
        let mut connection = UnconnectedIbvConnection::new::<CQ_SIZE>(ibv_context)?;
        let mr =
            unsafe { UnconnectedSyncTransferMr::new(&mut connection, memory_ptr, memory_length)? };
        Ok(UnconnectedIbvSimpleUnit { connection, mr })
    }
}

impl<M: Mode> Connect for UnconnectedIbvSimpleUnit<M> {
    type ConnectionConfig = IbvSimpleUnitConnectionConfig<M>;
    type Connected = IbvSimpleUnit<M>;

    fn connection_config(&self) -> Self::ConnectionConfig {
        IbvSimpleUnitConnectionConfig {
            connection_config: self.connection.connection_config(),
            mr_connection_config: self.mr.connection_config(),
        }
    }

    fn connect(
        self,
        connection_config: Self::ConnectionConfig,
    ) -> std::io::Result<Self::Connected> {
        let connection = self
            .connection
            .connect(connection_config.connection_config)?;
        let mr = self.mr.connect(connection_config.mr_connection_config)?;
        Ok(IbvSimpleUnit { connection, mr })
    }
}
