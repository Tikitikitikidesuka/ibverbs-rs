use crate::synchronization::centralized::Role::*;
use crate::synchronization::interface::{IbBNodeSync, IbBNodeSyncBuilder};
use ibverbs::ibv_qp_type::IBV_QPT_RC;
use ibverbs::{
    CompletionQueue, Context, MemoryRegion, PreparedQueuePair, ProtectionDomain, QueuePair,
    QueuePairEndpoint,
};

pub struct IbBCentralizedNodeSyncStaticConfig {
    pub ib_context: Context,
    pub coordinator: bool,
    pub nodes: usize,
}

// Vector contains the endpoints of the rest of nodes
pub enum IbBCentralizedSyncConfig {
    Master(Vec<QueuePairEndpoint>),
    Slave(QueuePairEndpoint),
}

pub struct IbBCentralizedNodeSyncBuilder {
    cq: CompletionQueue,
    pd: ProtectionDomain,
    fabric: BuilderFabric,
}

enum BuilderFabric {
    Master {
        mr: MemoryRegion<Vec<u8>>,
        prepared_qps: Vec<PreparedQueuePair>,
    },
    Slave {
        prepared_qp: PreparedQueuePair,
    },
}

impl IbBNodeSyncBuilder for IbBCentralizedNodeSyncBuilder {
    type StaticConfig = IbBCentralizedNodeSyncStaticConfig;
    type DynamicConfig = IbBCentralizedSyncConfig;
    type IbBNodeSync = IbBCentralizedNodeSync;

    fn new(static_config: Self::StaticConfig) -> std::io::Result<Self> {
        let cq = static_config
            .ib_context
            .create_cq(static_config.nodes as i32, IbBCentralizedNodeSync::CQ_ID)?;
        let pd = static_config.ib_context.alloc_pd()?;

        let fabric = match static_config.coordinator {
            true => BuilderFabric::Master {
                mr: pd.allocate(static_config.nodes)?,
                prepared_qps: (0..static_config.nodes)
                    .map(|_| pd.create_qp(&cq, &cq, IBV_QPT_RC)?.build())
                    .collect::<Result<Vec<_>, _>>()?,
            },
            false => BuilderFabric::Slave {
                prepared_qp: pd.create_qp(&cq, &cq, IBV_QPT_RC)?.build()?,
            },
        };

        Ok(Self { cq, pd, fabric })
    }

    fn dynamic_config(&self) -> std::io::Result<Self::DynamicConfig> {
        Ok(match &self.fabric {
            BuilderFabric::Slave { prepared_qp } => {
                IbBCentralizedSyncConfig::Slave(prepared_qp.endpoint()?)
            }
            BuilderFabric::Master { prepared_qps, .. } => IbBCentralizedSyncConfig::Master(
                prepared_qps
                    .iter()
                    .map(|pqp| pqp.endpoint())
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        })

        // TODO: BASICALLY... :(
        // TODO: YOU NEED A DIFFERENT BUILDER FOR MASTER AND SLAVE
    }

    fn build(self, dynamic_config: Self::DynamicConfig) -> std::io::Result<Self::IbBNodeSync> {
        match self.fabric {
            BuilderFabric::Master { mr, prepared_qps } => NodeFabric::Master {
                mr, qps: prepared_qps.iter().map(|pqp| pqp.han)
            }
            BuilderFabric::Slave { prepared_qp } => {}
        }
    }
}

pub struct IbBCentralizedNodeSync {
    cq: CompletionQueue,
    pd: ProtectionDomain,
    fabric: NodeFabric,
}

enum NodeFabric {
    Master {
        mr: MemoryRegion<Vec<u8>>,
        qps: Vec<QueuePair>,
    },
    Slave {
        qp: QueuePair,
    },
}

// Qps are treated by index. First is for node with index 0
// which does not mean with rank_id zero but the lowest rank id
pub struct IbBNodeConnections {
    pd: ProtectionDomain,
    cq: CompletionQueue,
    qps: Vec<QueuePair>,
}

impl IbBCentralizedNodeSync {
    const CQ_ID: isize = 33333;

    pub fn new_master(node_connections: &mut IbBNodeConnections) -> std::io::Result<Self> {
        let mr = node_connections.pd.allocate(node_connections.qps.len())?;
        Ok(Self { role: Master(mr) })
    }

    pub fn new_slave(node_connections: &mut IbBNodeConnections) -> std::io::Result<Self> {
        Ok(Self { role: Slave })
    }
}

impl IbBNodeSync for IbBCentralizedNodeSync {
    fn wait_barrier(&mut self) -> std::io::Result<()> {
        match &mut self.role {
            Master(mr) => Self::wait_barrier_master(mr),
            Slave => Self::wait_barrier_slave(),
        }
    }
}

impl IbBCentralizedNodeSync {
    const READY_STATUS: u8 = 255;
    const WAIT_STATUS: u8 = 0;

    fn wait_barrier_master(mr: &mut MemoryRegion<Vec<u8>>) -> std::io::Result<()> {
        // Wait for all other nodes to be ready
        while !mr
            .inner()
            .iter()
            .all(|status| *status == Self::READY_STATUS)
        {
            std::hint::spin_loop();
        }

        // Inform other nodes of barrier end
        // for qp in qps {
        //     post_inform(qp);
        // }
        //

        todo!()
    }

    fn wait_barrier_slave() -> std::io::Result<()> {
        // Notify master node
        // inform(master_qp) -> wr;
        // wr.wait()?;

        // Wait for master's notification
        // master.read(imm_value) -> wr;
        // wr.wait();

        todo!()
    }
}
