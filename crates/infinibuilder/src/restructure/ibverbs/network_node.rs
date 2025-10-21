use crate::restructure::barrier::{RdmaNetworkBarrier, RdmaNetworkMemoryRegionComponent};
use crate::restructure::ibverbs::connection::IbvConnectionBuildError::MemoryRegionRegisterError;
use crate::restructure::ibverbs::connection::{
    IbvConnection, IbvConnectionBuildError, IbvConnectionBuilder, IbvConnectionEndpoint,
    IbvPreparedConnection,
};
use crate::restructure::ibverbs::memory_region::{IbvMemoryRegion, IbvRemoteMemoryRegion};
use crate::restructure::rdma_network_node::{
    RdmaNetworkGroup, RdmaNetworkNode, RdmaNetworkSelfGroup, RdmaNetworkSelfGroupConnection,
    RdmaNetworkSelfGroupConnections, RdmaNetworkTransport,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

const BARRIER_MEM_ID: &str = "barrier";

#[derive(Debug, Error)]
pub enum IbvNetworkNodeBuildError {
    #[error("Connection builder error: {0}")]
    ConnectionBuilderError(#[from] IbvConnectionBuildError),
    #[error("Barrier component memory register error: {0}")]
    BarrierMemoryRegisterError(String),
}

pub struct IbvNetworkNodeBuilder<RankId, IbvDevName, CqParams, NumConns, Barrier> {
    rank_id: RankId,
    ibv_device_name: IbvDevName,
    completion_queue_params: CqParams,
    num_connections: NumConns,
    barrier: Barrier,
}

#[derive(Debug, Clone)]
pub struct BuilderRankId {
    rank_id: usize,
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

impl IbvNetworkNodeBuilder<(), (), (), (), ()> {
    pub fn new() -> Self {
        Self {
            rank_id: (),
            ibv_device_name: (),
            completion_queue_params: (),
            num_connections: (),
            barrier: (),
        }
    }
}

impl<IbvDevName, CqParams, NumConns, PreparedBarrier>
    IbvNetworkNodeBuilder<(), IbvDevName, CqParams, NumConns, PreparedBarrier>
{
    pub fn rank_id(
        self,
        rank_id: usize,
    ) -> IbvNetworkNodeBuilder<BuilderRankId, IbvDevName, CqParams, NumConns, PreparedBarrier> {
        IbvNetworkNodeBuilder {
            rank_id: BuilderRankId { rank_id },
            ibv_device_name: self.ibv_device_name,
            completion_queue_params: self.completion_queue_params,
            num_connections: self.num_connections,
            barrier: self.barrier,
        }
    }
}

impl<RankId, CqParams, NumConns, PreparedBarrier>
    IbvNetworkNodeBuilder<RankId, (), CqParams, NumConns, PreparedBarrier>
{
    pub fn ibv_device(
        self,
        device_name: impl Into<String>,
    ) -> IbvNetworkNodeBuilder<RankId, BuilderIbvDeviceName, CqParams, NumConns, PreparedBarrier>
    {
        IbvNetworkNodeBuilder {
            rank_id: self.rank_id,
            ibv_device_name: BuilderIbvDeviceName {
                ibv_device_name: device_name.into(),
            },
            completion_queue_params: self.completion_queue_params,
            num_connections: self.num_connections,
            barrier: self.barrier,
        }
    }
}

impl<RankId, IbvDevName, NumConns, PreparedBarrier>
    IbvNetworkNodeBuilder<RankId, IbvDevName, (), NumConns, PreparedBarrier>
{
    pub fn cq_params(
        self,
        capacity: usize,
        cache_capacity: usize,
    ) -> IbvNetworkNodeBuilder<RankId, IbvDevName, BuilderCqParams, NumConns, PreparedBarrier> {
        IbvNetworkNodeBuilder {
            rank_id: self.rank_id,
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

impl<RankId, IbvDevName, CqParams, PreparedBarrier>
    IbvNetworkNodeBuilder<RankId, IbvDevName, CqParams, (), PreparedBarrier>
{
    pub fn num_connections(
        self,
        num_connections: usize,
    ) -> IbvNetworkNodeBuilder<RankId, IbvDevName, CqParams, BuilderNumConnections, PreparedBarrier>
    {
        IbvNetworkNodeBuilder {
            rank_id: self.rank_id,
            ibv_device_name: self.ibv_device_name,
            completion_queue_params: self.completion_queue_params,
            num_connections: BuilderNumConnections { num_connections },
            barrier: self.barrier,
        }
    }
}

impl<RankId, IbvDevName, CqParams, NumConnections>
    IbvNetworkNodeBuilder<RankId, IbvDevName, CqParams, NumConnections, ()>
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
    ) -> IbvNetworkNodeBuilder<
        RankId,
        IbvDevName,
        CqParams,
        NumConnections,
        BuilderBarrier<PreparedBarrier>,
    > {
        IbvNetworkNodeBuilder {
            rank_id: self.rank_id,
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
        BuilderRankId,
        BuilderIbvDeviceName,
        BuilderCqParams,
        BuilderNumConnections,
        BuilderBarrier<PreparedBarrier>,
    >
{
    pub fn build(
        mut self,
    ) -> Result<IbvPreparedNetworkNode<Barrier, PreparedBarrier>, IbvNetworkNodeBuildError> {
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
                BARRIER_MEM_ID, // To mark it as the barrier mem of this connection
                mem_ptr,
                mem_length,
            ));
        }

        let prepared_connections = connection_builders
            .into_iter()
            .map(|conn_builder| conn_builder.build())
            .collect::<Result<Vec<_>, _>>()?;

        Ok(IbvPreparedNetworkNode {
            rank_id: self.rank_id.rank_id,
            prepared_connections,
            prepared_barrier: self.barrier.barrier,
        })
    }
}

pub struct IbvPreparedNetworkNode<
    Barrier: RdmaNetworkBarrier,
    PreparedBarrier: RdmaNetworkMemoryRegionComponent<IbvMemoryRegion, IbvRemoteMemoryRegion, Registered = Barrier>,
> {
    rank_id: usize,
    prepared_connections: Vec<IbvPreparedConnection>,
    prepared_barrier: PreparedBarrier,
}

impl<
    Barrier: RdmaNetworkBarrier,
    PreparedBarrier: RdmaNetworkMemoryRegionComponent<IbvMemoryRegion, IbvRemoteMemoryRegion, Registered = Barrier>,
> IbvPreparedNetworkNode<Barrier, PreparedBarrier>
{
    pub fn endpoint(&self) -> IbvNetworkNodeEndpoint {
        IbvNetworkNodeEndpoint {
            rank_id: self.rank_id,
            connection_endpoints: self
                .prepared_connections
                .iter()
                .map(|conn| conn.endpoint())
                .collect(),
        }
    }

    pub fn connect(
        self,
        connection_config: IbvNetworkNodeEndpoint,
    ) -> Result<IbvNetworkNode<Barrier>, IbvNetworkNodeBuildError> {
        let connections = self
            .prepared_connections
            .into_iter()
            .zip(connection_config.connection_endpoints)
            .map(|(prepared_connection, endpoint)| prepared_connection.connect(endpoint))
            .collect::<Result<Vec<_>, _>>()?;

        let mrs = connections
            .iter()
            .enumerate()
            .map(|(idx, conn)| {
                Ok((
                    conn.local_mr(BARRIER_MEM_ID).ok_or(
                        IbvNetworkNodeBuildError::BarrierMemoryRegisterError(format!(
                            "Missing barrier local memory for conn {idx}"
                        )),
                    )?,
                    conn.remote_mr(BARRIER_MEM_ID).ok_or(
                        IbvNetworkNodeBuildError::BarrierMemoryRegisterError(format!(
                            "Missing barrier remote memory for conn {idx}"
                        )),
                    )?,
                ))
            })
            .collect::<Result<Vec<_>, IbvNetworkNodeBuildError>>()?;

        let barrier = self.prepared_barrier.registered_mrs(mrs).map_err(|error| {
            IbvNetworkNodeBuildError::BarrierMemoryRegisterError(format!("{error}"))
        })?;

        Ok(IbvNetworkNode {
            rank_id: self.rank_id,
            connections,
            barrier,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IbvNetworkNodeEndpoint {
    rank_id: usize,
    connection_endpoints: Vec<IbvConnectionEndpoint>,
}

#[derive(Debug, Error, Clone)]
pub enum IbvNetworkNodeEndpointGatherError {
    #[error("Missing connection for {conn} from node {node}")]
    ConnectionMissing { node: usize, conn: usize },
}

impl IbvNetworkNodeEndpoint {
    /// Takes a vector of network node endpoints and generates a new one
    /// containing a connection for each of the gathered nodes
    pub fn gather_endpoints<'a>(
        rank_id: usize,
        endpoints: impl IntoIterator<Item = &'a IbvNetworkNodeEndpoint>,
    ) -> Result<IbvNetworkNodeEndpoint, IbvNetworkNodeEndpointGatherError> {
        let connection_endpoints = endpoints
            .into_iter()
            .enumerate()
            .map(|(idx, node_endpoint)| {
                node_endpoint
                    .connection_endpoints
                    .get(rank_id)
                    .ok_or(IbvNetworkNodeEndpointGatherError::ConnectionMissing {
                        node: rank_id,
                        conn: idx,
                    })
                    .cloned()
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(IbvNetworkNodeEndpoint {
            rank_id,
            connection_endpoints,
        })
    }
}

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
