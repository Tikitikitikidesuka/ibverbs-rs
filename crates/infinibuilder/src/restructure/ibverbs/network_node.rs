use crate::restructure::barrier::{RdmaNetworkBarrier, RdmaNetworkMemoryRegionComponent};
use crate::restructure::ibverbs::connection::{
    IbvConnection, IbvConnectionBuildError, IbvConnectionBuilder,
};
use crate::restructure::ibverbs::memory_region::{IbvMemoryRegion, IbvRemoteMemoryRegion};
use crate::restructure::rdma_network_node::{
    RdmaNetworkGroup, RdmaNetworkNode, RdmaNetworkSelfGroup, RdmaNetworkSelfGroupConnection,
    RdmaNetworkSelfGroupConnections, RdmaNetworkTransport,
};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IbvNetworkNodeBuildError {
    #[error("Connection builder error: {0}")]
    ConnectionBuilderError(#[from] IbvConnectionBuildError),
    #[error("Barrier component memory register error: {0}")]
    BarrierMemoryRegisterError(String),
}

pub struct IbvNetworkNodeBuilder<IbvDevName, CqParams, NumConns, Barrier> {
    ibv_device_name: IbvDevName,
    completion_queue_params: CqParams,
    num_connections: NumConns,
    barrier: Barrier,
}

#[derive(Debug, Clone)]
pub struct BuilderIbvDeviceName {
    ibv_device_name: String,
}

#[derive(Debug, Clone)]
pub struct BuilderCqParams {
    capacity: usize,
    cache_capacity: usize,
}

#[derive(Debug, Clone)]
pub struct BuilderNumConnections {
    num_connections: usize,
}

#[derive(Debug)]
pub struct BuilderBarrier<PreparedBarrier> {
    barrier: PreparedBarrier,
}

impl IbvNetworkNodeBuilder<(), (), (), ()> {
    pub fn new() -> Self {
        Self {
            ibv_device_name: (),
            completion_queue_params: (),
            num_connections: (),
            barrier: (),
        }
    }
}

impl<CqParams, NumConns, PreparedBarrier>
    IbvNetworkNodeBuilder<(), CqParams, NumConns, PreparedBarrier>
{
    pub fn ibv_device(
        self,
        device_name: impl Into<String>,
    ) -> IbvNetworkNodeBuilder<BuilderIbvDeviceName, CqParams, NumConns, PreparedBarrier> {
        IbvNetworkNodeBuilder {
            ibv_device_name: BuilderIbvDeviceName {
                ibv_device_name: device_name.into(),
            },
            completion_queue_params: self.completion_queue_params,
            num_connections: self.num_connections,
            barrier: self.barrier,
        }
    }
}

impl<IbvDevName, NumConns, PreparedBarrier>
    IbvNetworkNodeBuilder<IbvDevName, (), NumConns, PreparedBarrier>
{
    pub fn cq_params(
        self,
        capacity: usize,
        cache_capacity: usize,
    ) -> IbvNetworkNodeBuilder<IbvDevName, BuilderCqParams, NumConns, PreparedBarrier> {
        IbvNetworkNodeBuilder {
            ibv_device_name: self.ibv_device_name,
            completion_queue_params: BuilderCqParams {
                capacity,
                cache_capacity,
            },
            num_connections: self.num_connections,
            barrier: self.barrier,
        }
    }
}

impl<IbvDevName, CqParams, PreparedBarrier>
    IbvNetworkNodeBuilder<IbvDevName, CqParams, (), PreparedBarrier>
{
    pub fn num_connections(
        self,
        num_connections: usize,
    ) -> IbvNetworkNodeBuilder<IbvDevName, CqParams, BuilderNumConnections, PreparedBarrier> {
        IbvNetworkNodeBuilder {
            ibv_device_name: self.ibv_device_name,
            completion_queue_params: self.completion_queue_params,
            num_connections: BuilderNumConnections { num_connections },
            barrier: self.barrier,
        }
    }
}

impl<IbvDevName, CqParams, NumConnections>
    IbvNetworkNodeBuilder<IbvDevName, CqParams, NumConnections, ()>
{
    pub fn barrier<
        Barrier: RdmaNetworkBarrier,
        PreparedBarrier: RdmaNetworkMemoryRegionComponent<
                IbvMemoryRegion,
                IbvRemoteMemoryRegion,
                Registered = Barrier,
            >,
    >(
        self,
        barrier: PreparedBarrier,
    ) -> IbvNetworkNodeBuilder<IbvDevName, CqParams, NumConnections, BuilderBarrier<PreparedBarrier>>
    {
        IbvNetworkNodeBuilder {
            ibv_device_name: self.ibv_device_name,
            completion_queue_params: self.completion_queue_params,
            num_connections: self.num_connections,
            barrier: BuilderBarrier { barrier },
        }
    }
}

impl<
    Barrier: RdmaNetworkBarrier,
    PreparedBarrier: RdmaNetworkMemoryRegionComponent<IbvMemoryRegion, IbvRemoteMemoryRegion, Registered = Barrier>,
>
    IbvNetworkNodeBuilder<
        BuilderIbvDeviceName,
        BuilderCqParams,
        BuilderNumConnections,
        BuilderBarrier<PreparedBarrier>,
    >
{
    pub fn build(mut self) -> Result<IbvPreparedNetworkNode, IbvNetworkNodeBuildError> {
        let connection_builder = IbvConnectionBuilder::new()
            .ibv_device(self.ibv_device_name.ibv_device_name)
            .cq_params(
                self.completion_queue_params.capacity,
                self.completion_queue_params.cache_capacity,
            );

        let mem_vec = self
            .barrier
            .barrier
            .memory(self.num_connections.num_connections);
        if mem_vec.len() != self.num_connections.num_connections {
            return Err(IbvNetworkNodeBuildError::BarrierMemoryRegisterError(
                format!(
                    "Non matching memory region number: {} regions returned by the barrier component for {} connections",
                    mem_vec.len(),
                    self.num_connections.num_connections
                ),
            ));
        }

        let mut connection_builders = Vec::with_capacity(self.num_connections.num_connections);
        for conn_idx in 0..self.num_connections.num_connections {
            let (mem_ptr, mem_length) = mem_vec[conn_idx];
            connection_builders.push(connection_builder.clone().register_mr(
                format!("{conn_idx}"),
                mem_ptr,
                mem_length,
            ));
        }

        todo!()
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
