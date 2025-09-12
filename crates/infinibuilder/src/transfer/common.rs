use crate::component::UnconnectedComponent;
use crate::transfer::request::TransferRequest;
use crate::transfer::unsafe_slice::UnsafeSlice;
use dashmap::DashMap;
use derivative::Derivative;
use ibverbs::{
    CompletionQueue, MemoryRegion, PreparedQueuePair, ProtectionDomain, QueuePair,
    QueuePairEndpoint, ibv_qp_type, ibv_wc,
};
use serde::{Deserialize, Serialize};
use std::ops::RangeBounds;
use std::sync::Arc;
use thiserror::Error;

pub struct TransferConfig {
    num_peers: usize,
    memory_region: UnsafeSlice<u8>,
}

impl TransferConfig {
    // Unsafe because it will unbind the memory region slice's lifetime
    pub unsafe fn new(num_peers: usize, memory_region: &[u8]) -> Self {
        Self {
            num_peers,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionOutputConfig {
    self_qp_endpoints: Vec<QueuePairEndpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInputConfig {
    peer_qp_endpoints: Vec<QueuePairEndpoint>,
}

#[derive(Debug, Error)]
pub enum ConnectionConfigGatherError {
    #[error("Peer with index {idx} is not in range (0..{num_peers})")]
    PeerIndexOutOfRange { idx: usize, num_peers: usize },
}

impl ConnectionInputConfig {
    pub fn gather_connection_config(
        peer_configs: impl IntoIterator<Item = ConnectionOutputConfig>,
        remote_idx: usize,
    ) -> Result<Self, ConnectionConfigGatherError> {
        use ConnectionConfigGatherError::*;

        let peer_qp_endpoints = peer_configs
            .into_iter()
            .map(|receiver_config| {
                receiver_config
                    .self_qp_endpoints
                    .get(remote_idx)
                    .ok_or(PeerIndexOutOfRange {
                        idx: remote_idx,
                        num_peers: receiver_config.self_qp_endpoints.len(),
                    })
                    .cloned()
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self { peer_qp_endpoints })
    }
}

pub struct Transfer {
    qps: Vec<QueuePair>,
    mr: MemoryRegion<UnsafeSlice<u8>>,
    pd: ProtectionDomain,
    cq: Arc<CompletionQueue>,
    next_wr_id: u64,
    completion_cache: Arc<DashMap<u64, ibv_wc>>,
}

impl UnconnectedTransfer {
    const CQ_SIZE_PER_NODE: usize = 5;

    pub fn new(ib_context: &ibverbs::Context, config: TransferConfig) -> std::io::Result<Self> {
        let cq = ib_context.create_cq((Self::CQ_SIZE_PER_NODE * config.num_peers) as i32, 0)?;
        let pd = ib_context.alloc_pd()?;
        let mr = pd.register(config.memory_region)?;
        let prepared_qps = (0..config.num_peers)
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
}

impl UnconnectedComponent for UnconnectedTransfer {
    type ConnectionOutputConfig = ConnectionOutputConfig;
    type ConnectionInputConfig = ConnectionInputConfig;
    type ConnectedComponent = Transfer;

    fn connection_config(&self) -> ConnectionOutputConfig {
        ConnectionOutputConfig {
            self_qp_endpoints: self.qp_endpoints.clone(),
        }
    }

    fn connect(self, connection_config: ConnectionInputConfig) -> std::io::Result<Transfer> {
        Ok(Transfer {
            cq: Arc::new(self.cq),
            pd: self.pd,
            mr: self.mr,
            qps: self
                .prepared_qps
                .into_iter()
                .zip(connection_config.peer_qp_endpoints)
                .map(|(pqp, slave_qp_endpoint)| pqp.handshake(slave_qp_endpoint))
                .collect::<Result<Vec<_>, _>>()?,
            next_wr_id: 0,
            completion_cache: Arc::new(DashMap::new()),
        })
    }
}

#[derive(Debug, Error)]
pub enum TransferError {
    #[error("Peer with index {idx} is not in range (0..{num_peers})")]
    PeerIndexOutOfRange { idx: usize, num_peers: usize },
    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

impl Transfer {
    pub fn post_send(
        &mut self,
        peer_idx: usize,
        memory_range: impl RangeBounds<usize>,
    ) -> Result<TransferRequest, TransferError> {
        use TransferError::*;

        let num_peers = self.qps.len();
        let peer_qp = self.qps.get_mut(peer_idx).ok_or(PeerIndexOutOfRange {
            idx: peer_idx,
            num_peers,
        })?;

        let wr_id = self.next_wr_id;
        unsafe { peer_qp.post_send(&[self.mr.slice(memory_range)], self.next_wr_id, None)? };
        self.next_wr_id += 1;

        Ok(TransferRequest::new(
            wr_id,
            self.cq.clone(),
            self.completion_cache.clone(),
        ))
    }

    pub fn post_receive(
        &mut self,
        peer_idx: usize,
        memory_range: impl RangeBounds<usize>,
    ) -> Result<TransferRequest, TransferError> {
        use TransferError::*;

        let num_peers = self.qps.len();
        let peer_qp = self.qps.get_mut(peer_idx).ok_or(PeerIndexOutOfRange {
            idx: peer_idx,
            num_peers,
        })?;

        let wr_id = self.next_wr_id;
        unsafe { peer_qp.post_receive(&[self.mr.slice(memory_range)], self.next_wr_id)? };
        self.next_wr_id += 1;

        Ok(TransferRequest::new(
            wr_id,
            self.cq.clone(),
            self.completion_cache.clone(),
        ))
    }

    pub fn wait_send(
        &mut self,
        peer_idx: usize,
        memory_range: impl RangeBounds<usize>,
    ) -> Result<ibv_wc, TransferError> {
        Ok(self.post_send(peer_idx, memory_range)?.wait()?)
    }

    pub fn wait_receive(
        &mut self,
        peer_idx: usize,
        memory_range: impl RangeBounds<usize>,
    ) -> Result<ibv_wc, TransferError> {
        Ok(self.post_receive(peer_idx, memory_range)?.wait()?)
    }
}
