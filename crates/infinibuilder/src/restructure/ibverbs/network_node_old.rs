use crate::restructure::barrier::{RdmaNetworkBarrier, RdmaNetworkMemoryRegionComponent};
use crate::restructure::ibverbs::connection::{
    BuilderCompletionQueue, BuilderContext, BuilderQueuePair, IbvConnection, IbvConnectionBuilder,
    IbvConnectionBuildError,
};
use crate::restructure::ibverbs::memory_region::IbvMemoryRegion;
use crate::restructure::rdma_network_node::{
    RdmaNetworkGroup, RdmaNetworkNode, RdmaNetworkSelfGroup, RdmaNetworkSelfGroupConnection,
    RdmaNetworkSelfGroupConnections, RdmaNetworkTransport,
};
use ibverbs::{ProtectionDomain, RemoteMemoryRegion};
use std::error::Error;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IbvNetworkNodeBuilderError {
    #[error("Connection builder error: {0}")]
    ConnectionBuilderError(#[from] IbvConnectionBuildError),
    #[error("Barrier component memory register error: {0}")]
    BarrierMemoryRegisterError(String),
}

pub struct IbvNetworkNodeBuilder<IB, VB, CQP, NB, MRS> {
    builder: IB,
    builders: VB,
    cq_params: CQP,
    barrier: NB,
    mrs: MRS,
}

pub struct CompletionQueueParams {
    capacity: i32,
    cache_capacity: usize,
}

impl IbvNetworkNodeBuilder<(), (), (), (), ()> {
    pub fn new() -> Self {
        Self {
            builder: (),
            builders: (),
            cq_params: (),
            barrier: (),
            mrs: (),
        }
    }

    pub fn with_ibv_device(
        self,
        device_name: impl Into<String>,
    ) -> Result<
        IbvNetworkNodeBuilder<IbvConnectionBuilder<BuilderContext, (), (), ()>, (), (), (), ()>,
        IbvNetworkNodeBuilderError,
    > {
        Ok(IbvNetworkNodeBuilder {
            builder: IbvConnectionBuilder::new().with_ibv_device(device_name)?,
            builders: self.builders,
            cq_params: self.cq_params,
            barrier: self.barrier,
            mrs: self.mrs,
        })
    }
}

impl IbvNetworkNodeBuilder<IbvConnectionBuilder<BuilderContext, (), (), ()>, (), (), (), ()> {
    pub fn set_completion_queue_params(
        self,
        capacity: i32,
        cache_capacity: usize,
    ) -> IbvNetworkNodeBuilder<
        IbvConnectionBuilder<BuilderContext, (), (), ()>,
        (),
        CompletionQueueParams,
        (),
        (),
    > {
        IbvNetworkNodeBuilder {
            builder: self.builder,
            builders: self.builders,
            cq_params: CompletionQueueParams {
                capacity,
                cache_capacity,
            },
            barrier: self.barrier,
            mrs: self.mrs,
        }
    }
}

impl
IbvNetworkNodeBuilder<
    IbvConnectionBuilder<BuilderContext, (), (), ()>,
    (),
    CompletionQueueParams,
    (),
    (),
>
{
    pub fn create_connections(
        self,
        num_connections: usize,
    ) -> Result<
        IbvNetworkNodeBuilder<
            IbvConnectionBuilder<BuilderContext, (), (), ()>,
            Vec<
                IbvConnectionBuilder<
                    BuilderContext,
                    BuilderQueuePair,
                    ProtectionDomain,
                    BuilderCompletionQueue,
                >,
            >,
            CompletionQueueParams,
            (),
            (),
        >,
        IbvNetworkNodeBuilderError,
    > {
        Ok(IbvNetworkNodeBuilder {
            builder: self.builder.clone(),
            builders: (0..num_connections)
                .into_iter()
                .map(|_| {
                    self.builder
                        .clone()
                        .create_pd()?
                        .create_cq(self.cq_params.capacity, self.cq_params.cache_capacity)?
                        .create_qp()
                })
                .collect::<Result<_, _>>()?,
            cq_params: self.cq_params,
            barrier: self.barrier,
            mrs: self.mrs,
        })
    }
}

impl
IbvNetworkNodeBuilder<
    IbvConnectionBuilder<BuilderContext, (), (), ()>,
    Vec<
        IbvConnectionBuilder<
            BuilderContext,
            BuilderQueuePair,
            ProtectionDomain,
            BuilderCompletionQueue,
        >,
    >,
    CompletionQueueParams,
    (),
    (),
>
{
    pub fn set_barrier<
        NB: RdmaNetworkBarrier,
        BE: Error,
        Comp: RdmaNetworkMemoryRegionComponent<
            IbvMemoryRegion,
            RemoteMemoryRegion,
            Registered = NB,
            RegisterError = BE,
        >,
    >(
        mut self,
        mut barrier: Comp,
    ) -> Result<
        IbvNetworkNodeBuilder<
            IbvConnectionBuilder<BuilderContext, (), (), ()>,
            Vec<
                IbvConnectionBuilder<
                    BuilderContext,
                    BuilderQueuePair,
                    ProtectionDomain,
                    BuilderCompletionQueue,
                >,
            >,
            CompletionQueueParams,
            Comp,
            Vec<IbvMemoryRegion>,
        >,
        IbvNetworkNodeBuilderError,
    > {
        let memory = barrier.memory(self.builders.len());
        let mrs = memory
            .into_iter()
            .zip(&mut self.builders)
            .into_iter()
            .enumerate()
            .map(|(idx, (memory, builder))| {
                builder.register_mr(idx.to_string(), memory.0, memory.1)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(IbvNetworkNodeBuilder {
            builder: self.builder,
            builders: self.builders,
            cq_params: self.cq_params,
            barrier,
            mrs,
        })
    }
}

pub struct IbvPreparedNetworkNode {}

pub struct IbvNetworkNode<NB> {
    rank_id: usize,
    connections: Vec<IbvConnection>,
    barrier: NB,
}

impl<NB: RdmaNetworkBarrier, NT: RdmaNetworkTransport> RdmaNetworkNode<NB, NT>
for IbvNetworkNode<NB>
{
    type Conn = IbvConnection;

    fn barrier<Group>(&mut self, group: &Group, timeout: Duration) -> Result<(), NB::Error>
    where
        Group: RdmaNetworkSelfGroup,
    {
        let group_conns = IbvNetworkSelfGroupConnections {
            self_idx: group.self_idx(),
            rank_ids: group.rank_ids(),
            connections: &mut self.connections,
        };

        self.barrier.barrier(group_conns, timeout)
    }
}

pub struct IbvNetworkSelfGroupConnections<'a> {
    self_idx: usize,
    rank_ids: &'a [usize],
    connections: &'a mut [IbvConnection],
}

impl<'network> RdmaNetworkGroup for IbvNetworkSelfGroupConnections<'network> {
    fn len(&self) -> usize {
        self.rank_ids.len()
    }

    fn rank_ids(&self) -> &[usize] {
        self.rank_ids
    }

    fn rank_id(&self, idx: usize) -> Option<usize> {
        self.rank_ids.get(idx).cloned()
    }
}

impl<'network> RdmaNetworkSelfGroup for IbvNetworkSelfGroupConnections<'network> {
    fn self_idx(&self) -> usize {
        self.self_idx
    }
}

impl<'network> RdmaNetworkSelfGroupConnections<'network, IbvConnection>
for IbvNetworkSelfGroupConnections<'network>
{
    fn connection_mut(
        &mut self,
        idx: usize,
    ) -> Option<RdmaNetworkSelfGroupConnection<IbvConnection>> {
        let peer_rank_id = *self.rank_ids.get(idx)?;
        let self_rank_id = self.rank_ids.get(self.self_idx).cloned();
        self.connections.get_mut(peer_rank_id).map(|conn| {
            match Some(peer_rank_id) == self_rank_id {
                true => RdmaNetworkSelfGroupConnection::SelfConnection,
                false => RdmaNetworkSelfGroupConnection::PeerConnection(peer_rank_id, conn),
            }
        })
    }
}
