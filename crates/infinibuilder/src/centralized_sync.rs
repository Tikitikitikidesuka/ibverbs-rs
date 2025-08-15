use crate::sync_component::SyncComponent;
use derivative::Derivative;
use ibverbs::ibv_qp_type::IBV_QPT_RC;
use ibverbs::{
    CompletionQueue, MemoryRegion, PreparedQueuePair, ProtectionDomain, QueuePair,
    QueuePairEndpoint, RemoteMemoryRegion, RemoteMemorySlice, ibv_wc, ibv_wc_opcode,
};
use serde::{Deserialize, Serialize};
use std::array::IntoIter;
use std::cmp::PartialEq;

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

#[derive(Debug, Copy, Clone)]
pub struct CentralizedSyncMasterConfig {
    num_slaves: usize,
}

#[derive(Debug, Copy, Clone)]
pub struct CentralizedSyncSlaveConfig {
    slave_idx: usize,
}

#[derive(Debug)]
pub enum UnconnectedCentralizedSync {
    Master(UnconnectedSyncMaster),
    Slave(UnconnectedSyncSlave),
}

// Infiniband component dropping order is important
#[derive(Derivative)]
#[derivative(Debug)]
struct UnconnectedSyncMaster {
    #[derivative(Debug = "ignore")]
    slave_prepared_qps: Vec<PreparedQueuePair>,
    slave_qp_endpoints: Vec<QueuePairEndpoint>,
    #[derivative(Debug = "ignore")]
    mr: MemoryRegion<Vec<u8>>,
    #[derivative(Debug = "ignore")]
    pd: ProtectionDomain,
    #[derivative(Debug = "ignore")]
    cq: CompletionQueue,
}

// Infiniband component dropping order is important
#[derive(Derivative)]
#[derivative(Debug)]
struct UnconnectedSyncSlave {
    slave_idx: usize,
    #[derivative(Debug = "ignore")]
    master_prepared_qp: PreparedQueuePair,
    master_qp_endpoint: QueuePairEndpoint,
    #[derivative(Debug = "ignore")]
    mr: MemoryRegion<Vec<u8>>,
    #[derivative(Debug = "ignore")]
    pd: ProtectionDomain,
    #[derivative(Debug = "ignore")]
    cq: CompletionQueue,
}

#[derive(Debug, Clone, Serialize)]
pub enum CentralizedSyncConnectionOutputConfig {
    Master(MasterConnectionOutputConfig),
    Slave(SlaveConnectionOutputConfig),
}

#[derive(Debug, Clone, Deserialize)]
pub enum CentralizedSyncConnectionInputConfig {
    Master(MasterConnectionInputConfig),
    Slave(SlaveConnectionInputConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterConnectionOutputConfig {
    self_qp_endpoints: Vec<QueuePairEndpoint>,
    self_mr: RemoteMemoryRegion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaveConnectionOutputConfig {
    self_qp_endpoint: QueuePairEndpoint,
    self_mr: RemoteMemoryRegion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterConnectionInputConfig {
    slave_qp_endpoints: Vec<QueuePairEndpoint>,
    slave_mrs: Vec<RemoteMemoryRegion>,
}

pub struct CentralizedSyncConfigGatherer;

impl CentralizedSyncConfigGatherer {
    pub fn gather_master_config(
        slave_configs: impl IntoIterator<Item = SlaveConnectionOutputConfig>,
    ) -> MasterConnectionInputConfig {
        let (slave_qp_endpoints, slave_mrs) = slave_configs
            .into_iter()
            .map(|slave_config| (slave_config.self_qp_endpoint, slave_config.self_mr))
            .unzip();

        MasterConnectionInputConfig {
            slave_qp_endpoints,
            slave_mrs,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaveConnectionInputConfig {
    master_qp_endpoints: Vec<QueuePairEndpoint>,
    master_mr: RemoteMemoryRegion,
}

pub enum CentralizedSync {
    Master(ConnectedSyncMaster),
    Slave(ConnectedSyncSlave),
}

// Infiniband component dropping order is important
pub struct ConnectedSyncMaster {
    slave_qps: Vec<QueuePair>,
    slave_mrs: Vec<RemoteMemorySlice>,
    mr: MemoryRegion<Vec<u8>>,
    pd: ProtectionDomain,
    cq: CompletionQueue,
}

// Infiniband component dropping order is important
pub struct ConnectedSyncSlave {
    master_qp: QueuePair,
    master_mr: RemoteMemorySlice,
    mr: MemoryRegion<Vec<u8>>,
    pd: ProtectionDomain,
    cq: CompletionQueue,
}

impl UnconnectedCentralizedSync {
    pub fn new(
        ib_context: ibverbs::Context,
        network_config: CentralizedSyncConfig,
    ) -> std::io::Result<Self> {
        match network_config {
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

impl UnconnectedSyncMaster {
    const CQ_SIZE_PER_NODE: usize = 5;

    pub fn new(
        ib_context: ibverbs::Context,
        config: CentralizedSyncMasterConfig,
    ) -> std::io::Result<Self> {
        let cq = ib_context.create_cq((Self::CQ_SIZE_PER_NODE * config.num_slaves) as i32, 0)?;
        let pd = ib_context.alloc_pd()?;
        let mr = pd.allocate(config.num_slaves)?;
        let slave_prepared_qps = (0..config.num_slaves)
            .into_iter()
            .map(|_| pd.create_qp(&cq, &cq, IBV_QPT_RC)?.build())
            .collect::<Result<Vec<_>, _>>()?;
        let slave_qp_endpoints = slave_prepared_qps
            .iter()
            .map(|pqp| pqp.endpoint())
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            cq,
            pd,
            mr,
            slave_prepared_qps,
            slave_qp_endpoints,
        })
    }

    pub fn connection_config(&self) -> MasterConnectionOutputConfig {
        MasterConnectionOutputConfig {
            self_qp_endpoints: self.slave_qp_endpoints.clone(),
            self_mr: self.mr.remote(),
        }
    }

    pub fn connect(
        self,
        connection_config: MasterConnectionInputConfig,
    ) -> std::io::Result<ConnectedSyncMaster> {
        Ok(ConnectedSyncMaster {
            cq: self.cq,
            pd: self.pd,
            mr: self.mr,
            slave_qps: self
                .slave_prepared_qps
                .into_iter()
                .zip(connection_config.slave_qp_endpoints)
                .map(|(pqp, slave_qp_endpoint)| pqp.handshake(slave_qp_endpoint))
                .collect::<Result<Vec<_>, _>>()?,
            slave_mrs: connection_config
                .slave_mrs
                .iter()
                .map(|mr| mr.slice(0..1))
                .collect(),
        })
    }
}

impl UnconnectedSyncSlave {
    const CQ_SIZE: usize = 16;

    pub fn new(
        ib_context: ibverbs::Context,
        config: CentralizedSyncSlaveConfig,
    ) -> std::io::Result<Self> {
        let cq = ib_context.create_cq(Self::CQ_SIZE as i32, 0)?;
        let pd = ib_context.alloc_pd()?;
        let mr = pd.allocate(1)?;
        let master_prepared_qp = pd.create_qp(&cq, &cq, IBV_QPT_RC)?.build()?;
        let master_qp_endpoint = master_prepared_qp.endpoint()?;

        Ok(Self {
            cq,
            pd,
            mr,
            master_prepared_qp,
            master_qp_endpoint,
            slave_idx: config.slave_idx,
        })
    }

    pub fn connection_config(&self) -> SlaveConnectionOutputConfig {
        SlaveConnectionOutputConfig {
            self_qp_endpoint: self.master_qp_endpoint.clone(),
            self_mr: self.mr.remote(),
        }
    }

    pub fn connect(
        self,
        connection_config: SlaveConnectionInputConfig,
    ) -> std::io::Result<ConnectedSyncSlave> {
        Ok(ConnectedSyncSlave {
            cq: self.cq,
            pd: self.pd,
            mr: self.mr,
            master_qp: self.master_prepared_qp.handshake(
                *connection_config
                    .master_qp_endpoints
                    .get(self.slave_idx)
                    .ok_or(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!(
                            "Master did not send queue pair endpoint for slave with index {}",
                            self.slave_idx
                        ),
                    ))?,
            )?,
            master_mr: connection_config
                .master_mr
                .slice(self.slave_idx..self.slave_idx + 1),
        })
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

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, num_enum::IntoPrimitive, num_enum::TryFromPrimitive)]
enum NodeReadyStatus {
    NotReady,
    Ready,
}

impl NodeReadyStatus {
    fn from_raw(raw_status: u8) -> std::io::Result<Self> {
        Self::try_from(raw_status).map_err(|error| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid slave status detected in master's memory region: {error}"),
            )
        })
    }
}

impl SyncComponent for ConnectedSyncMaster {
    fn wait_barrier(&mut self) -> std::io::Result<()> {
        self._wait_for_slaves()?;
        self._reset_barrier_memory()?;
        self._notify_slaves()?;
        self._wait_for_slave_notification_completion()?;
        Ok(())
    }
}

impl ConnectedSyncMaster {
    const MASTER_WR_WRITE_ID: u64 = 0xDEADBEEF;

    // Wait for slaves to write they are ready into their memory region
    fn _wait_for_slaves(&mut self) -> std::io::Result<()> {
        let mut all_ready = false;

        while !all_ready {
            all_ready = true;

            for raw_status in self.mr.inner().iter() {
                if NodeReadyStatus::from_raw(*raw_status)? != NodeReadyStatus::Ready {
                    all_ready = false;
                    break;
                }
            }

            std::hint::spin_loop();
        }

        Ok(())
    }

    // Set all the memory to not ready for the next barrier
    fn _reset_barrier_memory(&mut self) -> std::io::Result<()> {
        self.mr
            .inner()
            .iter_mut()
            .for_each(|status| *status = NodeReadyStatus::NotReady.into());

        Ok(())
    }

    // Write not ready again on the slaves mr to notify them
    fn _notify_slaves(&mut self) -> std::io::Result<()> {
        self.slave_qps
            .iter_mut()
            .zip(self.slave_mrs.iter())
            .enumerate()
            .try_for_each(|(idx, (qp, mr))| {
                qp.post_write(
                    &[self.mr.slice(idx..idx + 1)],
                    *mr,
                    Self::MASTER_WR_WRITE_ID,
                    None,
                )
            })
    }

    // Wait for all writes to finish
    fn _wait_for_slave_notification_completion(&mut self) -> std::io::Result<()> {
        let mut completions = 0;
        let expected = self.slave_qps.len();
        let mut wc_buff = [ibv_wc::default(); 32];

        while completions < expected {
            for wc in self.cq.poll(&mut wc_buff)? {
                // Check that wc belongs to send wr
                if wc.opcode() == ibv_wc_opcode::IBV_WC_RDMA_WRITE
                    && wc.wr_id() == Self::MASTER_WR_WRITE_ID
                {
                    // Check it finished successfully
                    if !wc.is_valid() {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("Master to slave RDMA write notification failed: {:?}", wc),
                        ));
                    } else {
                        completions += 1;
                    }
                }
            }

            std::hint::spin_loop();
        }

        Ok(())
    }
}

impl SyncComponent for ConnectedSyncSlave {
    fn wait_barrier(&mut self) -> std::io::Result<()> {
        self._set_barrier_memory()?;
        self._notify_master()?;
        self._wait_for_master_notification_completion()?;
        self._wait_for_master()?;
        Ok(())
    }
}

impl ConnectedSyncSlave {
    const SLAVE_WR_WRITE_ID: u64 = 0xC0FFEE01;

    pub fn _set_barrier_memory(&mut self) -> std::io::Result<()> {
        self.mr.inner()[0] = NodeReadyStatus::Ready.into();

        Ok(())
    }

    // Write ready status to master
    pub fn _notify_master(&mut self) -> std::io::Result<()> {
        self.master_qp.post_write(
            &[self.mr.slice(0..1)],
            self.master_mr,
            Self::SLAVE_WR_WRITE_ID,
            None,
        )
    }

    // Wait for write completion
    fn _wait_for_master_notification_completion(&mut self) -> std::io::Result<()> {
        let mut done = false;
        let mut wc_buff = [ibv_wc::default(); 32];

        while !done {
            for wc in self.cq.poll(&mut wc_buff)? {
                // Check that wc belongs to send wr
                if wc.opcode() == ibv_wc_opcode::IBV_WC_RDMA_WRITE
                    && wc.wr_id() == Self::SLAVE_WR_WRITE_ID
                {
                    // Check it finished successfully
                    if !wc.is_valid() {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("Slave to master RDMA write notification failed: {:?}", wc),
                        ));
                    } else {
                        done = true;
                    }
                }
            }

            std::hint::spin_loop();
        }

        Ok(())
    }

    // Wait for master's reply by waiting for NotReady in our slice
    fn _wait_for_master(&mut self) -> std::io::Result<()> {
        while NodeReadyStatus::from_raw(self.mr.inner()[0])? != NodeReadyStatus::NotReady {
            std::hint::spin_loop();
        }

        Ok(())
    }
}
