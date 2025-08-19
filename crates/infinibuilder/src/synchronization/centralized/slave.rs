use derivative::Derivative;
use ibverbs::ibv_qp_type::IBV_QPT_RC;
use ibverbs::{
    CompletionQueue, MemoryRegion, PreparedQueuePair, ProtectionDomain, QueuePair,
    QueuePairEndpoint, RemoteMemoryRegion, RemoteMemorySlice, ibv_access_flags, ibv_wc,
    ibv_wc_opcode,
};
use serde::{Deserialize, Serialize};
use crate::synchronization::centralized::common::NodeReadyStatus;
use crate::synchronization::centralized::master::MasterConnectionOutputConfig;
use crate::synchronization::SyncComponent;

#[derive(Debug, Copy, Clone)]
pub struct CentralizedSyncSlaveConfig {
    pub(crate) slave_idx: usize,
}

// Infiniband component dropping order is important
#[derive(Derivative)]
#[derivative(Debug)]
pub struct UnconnectedSyncSlave {
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


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaveConnectionOutputConfig {
    pub(crate) self_qp_endpoint: QueuePairEndpoint,
    pub(crate) self_mr: RemoteMemoryRegion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaveConnectionInputConfig {
    pub(crate) master_qp_endpoints: Vec<QueuePairEndpoint>,
    pub(crate) master_mr: RemoteMemoryRegion,
}

impl SlaveConnectionInputConfig {
    pub fn adapt_slave_config(
        master_config: MasterConnectionOutputConfig,
    ) -> SlaveConnectionInputConfig {
        SlaveConnectionInputConfig {
            master_qp_endpoints: master_config.self_qp_endpoints,
            master_mr: master_config.self_mr,
        }
    }
}

// Infiniband component dropping order is important
pub struct ConnectedSyncSlave {
    master_qp: QueuePair,
    master_mr: RemoteMemorySlice,
    mr: MemoryRegion<Vec<u8>>,
    pd: ProtectionDomain,
    cq: CompletionQueue,
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
        let master_prepared_qp = pd
            .create_qp(&cq, &cq, IBV_QPT_RC)?
            .set_access(
                ibv_access_flags::IBV_ACCESS_REMOTE_WRITE
                    | ibv_access_flags::IBV_ACCESS_LOCAL_WRITE,
            )
            .build()?;
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
