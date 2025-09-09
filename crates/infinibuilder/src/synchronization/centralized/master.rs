use derivative::Derivative;
use ibverbs::{ibv_access_flags, ibv_qp_type, ibv_wc, ibv_wc_opcode, CompletionQueue, MemoryRegion, PreparedQueuePair, ProtectionDomain, QueuePair, QueuePairEndpoint, RemoteMemoryRegion, RemoteMemorySlice};
use serde::{Deserialize, Serialize};
use crate::synchronization::centralized::common::NodeReadyStatus;
use crate::synchronization::centralized::slave::SlaveConnectionOutputConfig;
use crate::synchronization::SyncComponent;

#[derive(Debug, Copy, Clone)]
pub struct CentralizedSyncMasterConfig {
    pub(crate) num_slaves: usize,
}

// Infiniband component dropping order is important
#[derive(Derivative)]
#[derivative(Debug)]
pub struct UnconnectedSyncMaster {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterConnectionOutputConfig {
    pub(crate) self_qp_endpoints: Vec<QueuePairEndpoint>,
    pub(crate) self_mr: RemoteMemoryRegion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterConnectionInputConfig {
    pub(crate) slave_qp_endpoints: Vec<QueuePairEndpoint>,
    pub(crate) slave_mrs: Vec<RemoteMemoryRegion>,
}

impl MasterConnectionInputConfig {
    pub fn gather_master_config(
        slave_configs: impl IntoIterator<Item = SlaveConnectionOutputConfig>,
    ) -> MasterConnectionInputConfig {
        let (slave_qp_endpoints, slave_mrs) = slave_configs
            .into_iter()
            .map(|slave_config| (slave_config.self_qp_endpoint, slave_config.self_mr))
            .unzip();

        Self {
            slave_qp_endpoints,
            slave_mrs,
        }
    }
}

// Infiniband component dropping order is important
pub struct ConnectedSyncMaster {
    slave_qps: Vec<QueuePair>,
    slave_mrs: Vec<RemoteMemorySlice>,
    mr: MemoryRegion<Vec<u8>>,
    pd: ProtectionDomain,
    cq: CompletionQueue,
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
            .map(|_| {
                pd.create_qp(&cq, &cq, ibv_qp_type::IBV_QPT_RC)?
                    .set_access(
                        ibv_access_flags::IBV_ACCESS_REMOTE_WRITE
                            | ibv_access_flags::IBV_ACCESS_LOCAL_WRITE,
                    )
                    .build()
            })
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
