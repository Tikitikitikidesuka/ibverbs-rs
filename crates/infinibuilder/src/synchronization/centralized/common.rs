use serde::{Deserialize, Serialize};
use crate::synchronization::centralized::master::{CentralizedSyncMasterConfig, ConnectedSyncMaster, MasterConnectionInputConfig, MasterConnectionOutputConfig, UnconnectedSyncMaster};
use crate::synchronization::centralized::slave::{CentralizedSyncSlaveConfig, ConnectedSyncSlave, SlaveConnectionInputConfig, SlaveConnectionOutputConfig, UnconnectedSyncSlave};
use crate::synchronization::SyncComponent;

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, num_enum::IntoPrimitive, num_enum::TryFromPrimitive)]
pub(crate) enum NodeReadyStatus {
    NotReady,
    Ready,
}

impl NodeReadyStatus {
    pub(crate) fn from_raw(raw_status: u8) -> std::io::Result<Self> {
        Self::try_from(raw_status).map_err(|error| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid slave status detected in master's memory region: {error}"),
            )
        })
    }
}

#[derive(Debug, Copy, Clone)]
pub enum CentralizedSyncConfig {
    Master(CentralizedSyncMasterConfig),
    Slave(CentralizedSyncSlaveConfig),
}

impl CentralizedSyncConfig {
    pub fn new_master(num_slaves: usize) -> Self {
        Self::Master(CentralizedSyncMasterConfig { num_slaves })
    }

    pub fn new_slave(slave_idx: usize) -> Self {
        Self::Slave(CentralizedSyncSlaveConfig { slave_idx })
    }
}

#[derive(Debug)]
pub enum UnconnectedCentralizedSync {
    Master(UnconnectedSyncMaster),
    Slave(UnconnectedSyncSlave),
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CentralizedSyncConnectionOutputConfig {
    Master(MasterConnectionOutputConfig),
    Slave(SlaveConnectionOutputConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CentralizedSyncConnectionInputConfig {
    Master(MasterConnectionInputConfig),
    Slave(SlaveConnectionInputConfig),
}

pub enum CentralizedSync {
    Master(ConnectedSyncMaster),
    Slave(ConnectedSyncSlave),
}

impl UnconnectedCentralizedSync {
    pub fn new(
        ib_context: ibverbs::Context,
        config: CentralizedSyncConfig,
    ) -> std::io::Result<Self> {
        match config {
            CentralizedSyncConfig::Master(config) => Ok(Self::Master(UnconnectedSyncMaster::new(
                ib_context, config,
            )?)),
            CentralizedSyncConfig::Slave(config) => {
                Ok(Self::Slave(UnconnectedSyncSlave::new(ib_context, config)?))
            }
        }
    }

    pub fn connection_config(&self) -> CentralizedSyncConnectionOutputConfig {
        match self {
            UnconnectedCentralizedSync::Master(master) => {
                CentralizedSyncConnectionOutputConfig::Master(master.connection_config())
            }
            UnconnectedCentralizedSync::Slave(slave) => {
                CentralizedSyncConnectionOutputConfig::Slave(slave.connection_config())
            }
        }
    }

    pub fn connect(
        self,
        connection_config: CentralizedSyncConnectionInputConfig,
    ) -> std::io::Result<CentralizedSync> {
        match (self, connection_config) {
            (
                UnconnectedCentralizedSync::Master(master),
                CentralizedSyncConnectionInputConfig::Master(config),
            ) => Ok(CentralizedSync::Master(master.connect(config)?)),
            (
                UnconnectedCentralizedSync::Slave(slave),
                CentralizedSyncConnectionInputConfig::Slave(config),
            ) => Ok(CentralizedSync::Slave(slave.connect(config)?)),
            (node, config) => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "Connection config mismatch: node = {:?}, config = {:?}",
                    node, config
                ),
            )),
        }
    }
}

impl SyncComponent for CentralizedSync {
    fn wait_barrier(&mut self) -> std::io::Result<()> {
        match self {
            CentralizedSync::Master(master) => master.wait_barrier(),
            CentralizedSync::Slave(slave) => slave.wait_barrier(),
        }
    }
}


