pub mod request;

mod unsafe_slice;

use crate::transfer::request::TransferRequest;
use crate::transfer::unsafe_slice::UnsafeSlice;
use derivative::Derivative;
use ibverbs::{CompletionQueue, MemoryRegion, PreparedQueuePair, ProtectionDomain, QueuePairEndpoint, ibv_qp_type, ibv_wc, QueuePair, RemoteMemorySlice};
use std::ops::RangeBounds;

pub struct TransferConfig {
    num_nodes: usize,
    memory_region: UnsafeSlice<u8>,
}

impl TransferConfig {
    // Unsafe because it will unbind the memory region slice's lifetime
    pub unsafe fn new(num_nodes: usize, memory_region: &[u8]) -> Self {
        Self {
            num_nodes,
            memory_region: unsafe { UnsafeSlice::new(memory_region) },
        }
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct UnconnectedTransfer {
    #[derivative(Debug = "ignore")]
    prepared_qps: Vec<PreparedQueuePair>,
    qp_endpoints: Vec<QueuePairEndpoint>,
    #[derivative(Debug = "ignore")]
    mr: MemoryRegion<UnsafeSlice<u8>>,
    #[derivative(Debug = "ignore")]
    pd: ProtectionDomain,
    #[derivative(Debug = "ignore")]
    cq: CompletionQueue,
}

impl UnconnectedTransfer {
    const CQ_SIZE_PER_NODE: usize = 5;

    pub fn new(ib_context: ibverbs::Context, config: TransferConfig) -> std::io::Result<Self> {
        let cq = ib_context.create_cq((Self::CQ_SIZE_PER_NODE * config.num_nodes) as i32, 0)?;
        let pd = ib_context.alloc_pd()?;
        let mr = pd.register(config.memory_region)?;
        let prepared_qps = (0..config.num_nodes)
            .into_iter()
            .map(|_| pd.create_qp(&cq, &cq, ibv_qp_type::IBV_QPT_RC)?.build())
            .collect::<Result<Vec<_>, _>>()?;
        let qp_endpoints = prepared_qps
            .iter()
            .map(|pqp| pqp.endpoint())
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            cq,
            pd,
            mr,
            prepared_qps,
            qp_endpoints,
        })
    }

    pub fn connection_config(&self) -> ConnectionOutputConfig {
        ConnectionOutputConfig {
            self_qp_endpoints: self.qp_endpoints.clone(),
        }
    }

    pub fn connect(
        self,
        connection_config: ConnectionInputConfig,
    ) -> std::io::Result<ConnectedTransfer> {
        Ok(ConnectedTransfer {
            cq: self.cq,
            pd: self.pd,
            mr: self.mr,
            qps: self
                .prepared_qps
                .into_iter()
                .zip(connection_config.remote_qp_endpoints)
                .map(|(pqp, slave_qp_endpoint)| pqp.handshake(slave_qp_endpoint))
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

pub struct ConnectionOutputConfig {
    self_qp_endpoints: Vec<QueuePairEndpoint>,
}

pub struct ConnectionInputConfig {
    remote_qp_endpoints: Vec<QueuePairEndpoint>,
}

pub struct ConnectedTransfer {
    qps: Vec<QueuePair>,
    mr: MemoryRegion<UnsafeSlice<u8>>,
    pd: ProtectionDomain,
    cq: CompletionQueue,
}

impl ConnectedTransfer {
    fn post_send(
        &self,
        rank_id: u32,
        memory_range: impl RangeBounds<usize>,
    ) -> std::io::Result<TransferRequest> {
        todo!()
    }

    fn post_receive(
        &self,
        rank_id: u32,
        memory_range: impl RangeBounds<usize>,
    ) -> std::io::Result<TransferRequest> {
        todo!()
    }

    fn wait_send(
        &self,
        rank_id: u32,
        memory_range: impl RangeBounds<usize>,
    ) -> std::io::Result<ibv_wc> {
        self.post_send(rank_id, memory_range)?.wait()
    }

    fn wait_receive(
        &self,
        rank_id: u32,
        memory_range: impl RangeBounds<usize>,
    ) -> std::io::Result<ibv_wc> {
        self.post_receive(rank_id, memory_range)?.wait()
    }
}
