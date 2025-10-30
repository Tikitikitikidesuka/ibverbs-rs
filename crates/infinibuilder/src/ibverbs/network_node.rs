use crate::barrier::{MemoryRegionPair, RdmaNetworkBarrier, RdmaNetworkMemoryRegionComponent};
use crate::ibverbs::connection::{
    IbvConnection, IbvConnectionBuildError, IbvConnectionBuilder, IbvConnectionEndpoint,
    IbvMemoryRegion, IbvPostError, IbvPreparedConnection, IbvRemoteMemoryRegion,
};
use crate::ibverbs::work_request::IbvWorkRequest;
use crate::rdma_connection::{
    RdmaImmediateDataConnection, RdmaNamedMemoryRegionConnection, RdmaReadWriteConnection,
    RdmaSendReceiveConnection,
};
use crate::rdma_network_node::{
    RdmaBarrierNetworkNode, RdmaGroupNetworkNode, RdmaNamedMemoryRegionNetworkNode,
    RdmaNetworkGroup, RdmaNetworkNode, RdmaNetworkSelfGroup, RdmaNetworkSelfGroupConnection,
    RdmaNetworkSelfGroupConnections, RdmaRankIdNetworkNode, RdmaTransportImmediateDataNetworkNode,
    RdmaTransportReadWriteNetworkNode, RdmaTransportSendReceiveNetworkNode,
};
use derivative::Derivative;
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::collections::HashMap;
use std::error::Error;
use std::ops::RangeBounds;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use crate::ibverbs::Named;

const BARRIER_MEM_ID: &str = "barrier";

#[derive(Debug, Error)]
pub enum IbvNetworkNodeBuildError {
    #[error("Connection builder error: {0}")]
    ConnectionBuilderError(#[from] IbvConnectionBuildError),
    #[error("Barrier component memory register error: {0}")]
    BarrierMemoryRegisterError(String),
    #[error("Transport memory register error: {0}")]
    TransportMemoryRegisterError(String),
}

pub struct IbvNetworkNodeBuilder<RankId, IbvDevName, CqParams, NumConns, Barrier> {
    rank_id: RankId,
    ibv_device_name: IbvDevName,
    completion_queue_params: CqParams,
    num_connections: NumConns,
    barrier: Barrier,
    transport_mrs: Vec<(String, *mut u8, usize)>,
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

#[derive(Debug)]
pub struct BuilderTransportMrs {
    mrs: Vec<(String, *mut u8, usize)>,
}

impl IbvNetworkNodeBuilder<(), (), (), (), ()> {
    pub fn new() -> Self {
        Self {
            rank_id: (),
            ibv_device_name: (),
            completion_queue_params: (),
            num_connections: (),
            barrier: (),
            transport_mrs: vec![],
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
            transport_mrs: self.transport_mrs,
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
            transport_mrs: self.transport_mrs,
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
            transport_mrs: self.transport_mrs,
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
            transport_mrs: self.transport_mrs,
        }
    }
}

impl<RankId, IbvDevName, CqParams, NumConnections>
    IbvNetworkNodeBuilder<RankId, IbvDevName, CqParams, NumConnections, ()>
{
    pub fn barrier<
        Barrier: RdmaNetworkBarrier<IbvMemoryRegion, IbvRemoteMemoryRegion>,
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
            transport_mrs: self.transport_mrs,
        }
    }
}

impl<RankId, IbvDevName, CqParams, NumConnections, PreparedBarrier>
    IbvNetworkNodeBuilder<RankId, IbvDevName, CqParams, NumConnections, PreparedBarrier>
{
    pub fn register_mr(
        mut self,
        id: impl Into<String>,
        mem_ptr: *mut u8,
        mem_length: usize,
    ) -> IbvNetworkNodeBuilder<RankId, IbvDevName, CqParams, NumConnections, PreparedBarrier> {
        self.transport_mrs.push((id.into(), mem_ptr, mem_length));

        IbvNetworkNodeBuilder {
            rank_id: self.rank_id,
            ibv_device_name: self.ibv_device_name,
            completion_queue_params: self.completion_queue_params,
            num_connections: self.num_connections,
            barrier: self.barrier,
            transport_mrs: self.transport_mrs,
        }
    }

    pub fn register_mrs(
        mut self,
        mrs: impl IntoIterator<Item = (impl Into<String>, *mut u8, usize)>,
    ) -> IbvNetworkNodeBuilder<RankId, IbvDevName, CqParams, NumConnections, PreparedBarrier> {
        let mut new_mrs = mrs
            .into_iter()
            .map(|(id, ptr, length)| (id.into(), ptr, length))
            .collect();
        self.transport_mrs.append(&mut new_mrs);

        IbvNetworkNodeBuilder {
            rank_id: self.rank_id,
            ibv_device_name: self.ibv_device_name,
            completion_queue_params: self.completion_queue_params,
            num_connections: self.num_connections,
            barrier: self.barrier,
            transport_mrs: self.transport_mrs,
        }
    }
}

impl<
    Barrier: RdmaNetworkBarrier<IbvMemoryRegion, IbvRemoteMemoryRegion>,
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
        // Create connections with device and cq config
        let connection_builder = IbvConnectionBuilder::new()
            .ibv_device(self.ibv_device_name.ibv_device_name)
            .cq_params(
                self.completion_queue_params.capacity,
                self.completion_queue_params.cache_capacity,
            );

        // Get memory regions from the barrier
        let barrier_mem_vec = self
            .barrier
            .barrier
            .memory(self.num_connections.num_connections);
        // Check the number of memory region matches the number of connections
        if barrier_mem_vec.len() != self.num_connections.num_connections {
            return Err(IbvNetworkNodeBuildError::BarrierMemoryRegisterError(
                format!(
                    "Non matching memory region number: {} regions returned by the barrier component for {} connections",
                    barrier_mem_vec.len(),
                    self.num_connections.num_connections
                ),
            ));
        }

        // Register the barrier memory regions in their corresponding connections
        let mut connection_builders = Vec::with_capacity(self.num_connections.num_connections);
        for conn_idx in 0..self.num_connections.num_connections {
            let (mem_ptr, mem_length) = barrier_mem_vec[conn_idx];
            connection_builders.push(connection_builder.clone().register_mr(
                BARRIER_MEM_ID, // To mark it as the barrier mem of this connection
                mem_ptr,
                mem_length,
            ));
        }

        // Register the transport memory regions on every connection
        let connection_builders = connection_builders
            .into_iter()
            .map(|conn_builder| conn_builder.register_mrs(self.transport_mrs.clone()))
            .collect::<Vec<_>>();

        let prepared_connections = connection_builders
            .into_iter()
            .map(|conn_builder| conn_builder.build())
            .collect::<Result<Vec<_>, _>>()?;

        Ok(IbvPreparedNetworkNode {
            rank_id: self.rank_id.rank_id,
            prepared_connections,
            prepared_barrier: self.barrier.barrier,
            transport_mrs: self.transport_mrs,
        })
    }
}

pub struct IbvPreparedNetworkNode<
    Barrier: RdmaNetworkBarrier<IbvMemoryRegion, IbvRemoteMemoryRegion>,
    PreparedBarrier: RdmaNetworkMemoryRegionComponent<IbvMemoryRegion, IbvRemoteMemoryRegion, Registered = Barrier>,
> {
    rank_id: usize,
    prepared_connections: Vec<IbvPreparedConnection>,
    prepared_barrier: PreparedBarrier,
    transport_mrs: Vec<(String, *mut u8, usize)>,
}

impl<
    Barrier: RdmaNetworkBarrier<IbvMemoryRegion, IbvRemoteMemoryRegion>,
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
        let greatest_rank_id = self.prepared_connections.len().saturating_sub(1);

        let connections = self
            .prepared_connections
            .into_iter()
            .zip(connection_config.connection_endpoints)
            .map(|(prepared_connection, endpoint)| prepared_connection.connect(endpoint))
            .collect::<Result<Vec<_>, _>>()?;

        let barrier_mrs = connections
            .iter()
            .enumerate()
            .map(|(idx, conn)| {
                Ok(MemoryRegionPair {
                    local_mr: conn.local_mr(BARRIER_MEM_ID).ok_or(
                        IbvNetworkNodeBuildError::BarrierMemoryRegisterError(format!(
                            "Missing barrier local memory for conn {idx}"
                        )),
                    )?,
                    remote_mr: conn.remote_mr(BARRIER_MEM_ID).ok_or(
                        IbvNetworkNodeBuildError::BarrierMemoryRegisterError(format!(
                            "Missing barrier remote memory for conn {idx}"
                        )),
                    )?,
                })
            })
            .collect::<Result<Vec<_>, IbvNetworkNodeBuildError>>()?;

        let barrier = self
            .prepared_barrier
            .registered_mrs(barrier_mrs)
            .map_err(|error| {
                IbvNetworkNodeBuildError::BarrierMemoryRegisterError(format!("{error}"))
            })?;

        let transport_mrs = self
            .transport_mrs
            .iter()
            .enumerate()
            .map(|(idx, (mr_id, mr_ptr, mr_length))| {
                // Mrs named `mr_id` of each connection
                let local_mrs: Vec<_> = connections
                    .iter()
                    .map(|conn| {
                        conn.local_mr(mr_id).ok_or(
                            IbvNetworkNodeBuildError::TransportMemoryRegisterError(format!(
                                "Missing transport local memory for conn {idx}"
                            )),
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                // Remote mrs named `mr_id` of each connection
                let remote_mrs: Vec<_> = connections
                    .iter()
                    .map(|conn| {
                        conn.remote_mr(mr_id).ok_or(
                            IbvNetworkNodeBuildError::TransportMemoryRegisterError(format!(
                                "Missing transport remote memory for conn {idx}"
                            )),
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                Ok((
                    mr_id.clone(),
                    MemoryRegionPair {
                        local_mr: IbvNetworkNodeMemoryRegion {
                            conn_mrs: Arc::new(Named::new(mr_id.clone(), local_mrs)),
                        },
                        remote_mr: IbvNetworkNodeRemoteMemoryRegion {
                            conn_mrs: Arc::new(Named::new(mr_id.clone(), remote_mrs)),
                        },
                    },
                ))
            })
            .collect::<Result<HashMap<_, _>, IbvNetworkNodeBuildError>>()?;

        Ok(IbvNetworkNode {
            rank_id: self.rank_id,
            greatest_rank_id,
            all_group: IbvNetworkSelfGroup {
                self_idx: self.rank_id,
                rank_ids: (0..=greatest_rank_id).collect(),
            },
            connections,
            barrier,
            transport_mrs,
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
    pub fn gather<'a>(
        rank_id: usize,
        endpoints: impl IntoIterator<Item = impl Borrow<IbvNetworkNodeEndpoint>>,
    ) -> Result<IbvNetworkNodeEndpoint, IbvNetworkNodeEndpointGatherError> {
        let connection_endpoints = endpoints
            .into_iter()
            .enumerate()
            .map(|(idx, node_endpoint)| {
                node_endpoint
                    .borrow()
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

#[derive(Derivative)]
#[derivative(Debug)]
pub struct IbvNetworkNode<NB> {
    rank_id: usize,
    greatest_rank_id: usize,
    all_group: IbvNetworkSelfGroup,
    connections: Vec<IbvConnection>,
    barrier: NB,
    #[derivative(Debug = "ignore")]
    transport_mrs: HashMap<
        String,
        MemoryRegionPair<IbvNetworkNodeMemoryRegion, IbvNetworkNodeRemoteMemoryRegion>,
    >,
}

#[derive(Debug, Clone)]
pub struct IbvNetworkNodeMemoryRegion {
    conn_mrs: Arc<Named<Vec<IbvMemoryRegion>>>,
}

#[derive(Debug, Clone)]
pub struct IbvNetworkNodeRemoteMemoryRegion {
    conn_mrs: Arc<Named<Vec<IbvRemoteMemoryRegion>>>,
}

impl<NB: RdmaNetworkBarrier<IbvMemoryRegion, IbvRemoteMemoryRegion>>
    RdmaNetworkNode<
        IbvNetworkNodeMemoryRegion,
        IbvNetworkNodeRemoteMemoryRegion,
        IbvMemoryRegion,
        IbvRemoteMemoryRegion,
        NB,
    > for IbvNetworkNode<NB>
{
}

impl<NB> RdmaRankIdNetworkNode for IbvNetworkNode<NB> {
    fn rank_id(&self) -> usize {
        self.rank_id
    }
}

impl<NB>
    RdmaNamedMemoryRegionNetworkNode<IbvNetworkNodeMemoryRegion, IbvNetworkNodeRemoteMemoryRegion>
    for IbvNetworkNode<NB>
{
    fn local_mr(&self, id: impl AsRef<str>) -> Option<IbvNetworkNodeMemoryRegion> {
        self.transport_mrs
            .get(id.as_ref())
            .map(|mr_pair| mr_pair.local_mr.clone())
    }

    fn remote_mr(&self, id: impl AsRef<str>) -> Option<IbvNetworkNodeRemoteMemoryRegion> {
        self.transport_mrs
            .get(id.as_ref())
            .map(|mr_pair| mr_pair.remote_mr.clone())
    }
}

impl<NB> RdmaGroupNetworkNode for IbvNetworkNode<NB> {
    type Group = IbvNetworkGroup;
    type SelfGroup = IbvNetworkSelfGroup;

    fn group_all(&self) -> Self::SelfGroup {
        self.all_group.clone()
    }

    fn group_peers(&self) -> Self::Group {
        todo!()
    }
}

impl<NB: RdmaNetworkBarrier<IbvMemoryRegion, IbvRemoteMemoryRegion>>
    RdmaBarrierNetworkNode<IbvMemoryRegion, IbvRemoteMemoryRegion, NB> for IbvNetworkNode<NB>
{
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

#[derive(Debug, Error)]
pub enum ConnectionTransportPostError<PostError: Error> {
    #[error("Invalid peer rank id {0}")]
    InvalidPeerRankId(usize),
    #[error("Transport to self is not allowed")]
    SelfPeerRankId,
    #[error("Rdma post error: {0}")]
    RdmaPostError(#[from] PostError),
}

impl<NB> RdmaTransportSendReceiveNetworkNode<IbvNetworkNodeMemoryRegion> for IbvNetworkNode<NB> {
    type WR = IbvWorkRequest;
    type PostError = ConnectionTransportPostError<std::io::Error>;

    fn post_send(
        &mut self,
        peer_rank_id: usize,
        memory_region: &IbvNetworkNodeMemoryRegion,
        memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<Self::WR, Self::PostError> {
        if peer_rank_id == self.all_group.self_rank_id() {
            return Err(ConnectionTransportPostError::SelfPeerRankId);
        }

        if peer_rank_id > self.greatest_rank_id {
            return Err(ConnectionTransportPostError::InvalidPeerRankId(
                peer_rank_id,
            ));
        }

        let connection = self.connections.get_mut(peer_rank_id).ok_or(
            ConnectionTransportPostError::InvalidPeerRankId(peer_rank_id),
        )?;

        let wr = connection.post_send(
            &memory_region.conn_mrs.data[peer_rank_id],
            memory_range,
            immediate_data,
        )?;

        Ok(wr)
    }

    fn post_receive(
        &mut self,
        peer_rank_id: usize,
        memory_region: &IbvNetworkNodeMemoryRegion,
        memory_range: impl RangeBounds<usize>,
    ) -> Result<Self::WR, Self::PostError> {
        if peer_rank_id == self.all_group.self_rank_id() {
            return Err(ConnectionTransportPostError::SelfPeerRankId);
        }

        if peer_rank_id > self.greatest_rank_id {
            return Err(ConnectionTransportPostError::InvalidPeerRankId(
                peer_rank_id,
            ));
        }

        let connection = self.connections.get_mut(peer_rank_id).ok_or(
            ConnectionTransportPostError::InvalidPeerRankId(peer_rank_id),
        )?;

        let wr = connection.post_receive(&memory_region.conn_mrs.data[peer_rank_id], memory_range)?;

        Ok(wr)
    }
}

impl<NB>
    RdmaTransportReadWriteNetworkNode<IbvNetworkNodeMemoryRegion, IbvNetworkNodeRemoteMemoryRegion>
    for IbvNetworkNode<NB>
{
    type WR = IbvWorkRequest;
    type PostError = ConnectionTransportPostError<std::io::Error>;

    fn post_write(
        &mut self,
        peer_rank_id: usize,
        local_memory_region: &IbvNetworkNodeMemoryRegion,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: &IbvNetworkNodeRemoteMemoryRegion,
        remote_memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<Self::WR, Self::PostError> {
        if peer_rank_id == self.all_group.self_rank_id() {
            return Err(ConnectionTransportPostError::SelfPeerRankId);
        }

        if peer_rank_id > self.greatest_rank_id {
            return Err(ConnectionTransportPostError::InvalidPeerRankId(
                peer_rank_id,
            ));
        }

        let connection = self.connections.get_mut(peer_rank_id).ok_or(
            ConnectionTransportPostError::InvalidPeerRankId(peer_rank_id),
        )?;

        let wr = connection.post_write(
            &local_memory_region.conn_mrs.data[peer_rank_id],
            local_memory_range,
            &remote_memory_region.conn_mrs.data[peer_rank_id],
            remote_memory_range,
            immediate_data,
        )?;

        Ok(wr)
    }

    fn post_read(
        &mut self,
        peer_rank_id: usize,
        local_memory_region: &IbvNetworkNodeMemoryRegion,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: &IbvNetworkNodeRemoteMemoryRegion,
        remote_memory_range: impl RangeBounds<usize>,
    ) -> Result<Self::WR, Self::PostError> {
        if peer_rank_id == self.all_group.self_rank_id() {
            return Err(ConnectionTransportPostError::SelfPeerRankId);
        }

        if peer_rank_id > self.greatest_rank_id {
            return Err(ConnectionTransportPostError::InvalidPeerRankId(
                peer_rank_id,
            ));
        }

        let connection = self.connections.get_mut(peer_rank_id).ok_or(
            ConnectionTransportPostError::InvalidPeerRankId(peer_rank_id),
        )?;

        let wr = connection.post_read(
            &local_memory_region.conn_mrs.data[peer_rank_id],
            local_memory_range,
            &remote_memory_region.conn_mrs.data[peer_rank_id],
            remote_memory_range,
        )?;

        Ok(wr)
    }
}

impl<NB> RdmaTransportImmediateDataNetworkNode for IbvNetworkNode<NB> {
    type WR = IbvWorkRequest;
    type PostError = ConnectionTransportPostError<std::io::Error>;

    fn post_send_immediate_data(
        &mut self,
        peer_rank_id: usize,
        immediate_data: u32,
    ) -> Result<Self::WR, Self::PostError> {
        if peer_rank_id == self.all_group.self_rank_id() {
            return Err(ConnectionTransportPostError::SelfPeerRankId);
        }

        let connection = self.connections.get_mut(peer_rank_id).ok_or(
            ConnectionTransportPostError::InvalidPeerRankId(peer_rank_id),
        )?;

        let wr = connection.post_send_immediate_data(immediate_data)?;

        Ok(wr)
    }

    fn post_receive_immediate_data(
        &mut self,
        peer_rank_id: usize,
    ) -> Result<Self::WR, Self::PostError> {
        if peer_rank_id == self.all_group.self_rank_id() {
            return Err(ConnectionTransportPostError::SelfPeerRankId);
        }

        let connection = self.connections.get_mut(peer_rank_id).ok_or(
            ConnectionTransportPostError::InvalidPeerRankId(peer_rank_id),
        )?;

        let wr = connection.post_receive_immediate_data()?;

        Ok(wr)
    }
}

#[derive(Debug, Clone)]
pub struct IbvNetworkGroup {
    rank_ids: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct IbvNetworkSelfGroup {
    self_idx: usize,
    rank_ids: Vec<usize>,
}

impl RdmaNetworkGroup for IbvNetworkGroup {
    fn len(&self) -> usize {
        self.rank_ids.len()
    }

    fn rank_ids(&self) -> &[usize] {
        self.rank_ids.as_slice()
    }

    fn rank_id(&self, idx: usize) -> Option<usize> {
        self.rank_ids.get(idx).copied()
    }
}

impl RdmaNetworkGroup for IbvNetworkSelfGroup {
    fn len(&self) -> usize {
        self.rank_ids.len()
    }

    fn rank_ids(&self) -> &[usize] {
        self.rank_ids.as_slice()
    }

    fn rank_id(&self, idx: usize) -> Option<usize> {
        self.rank_ids.get(idx).copied()
    }
}

impl RdmaNetworkSelfGroup for IbvNetworkSelfGroup {
    fn self_idx(&self) -> usize {
        self.self_idx
    }
}

impl<NB: RdmaNetworkBarrier<IbvMemoryRegion, IbvRemoteMemoryRegion>> IbvNetworkNode<NB> {
    pub fn group(&self, filter: impl FnMut(&usize) -> bool) -> IbvNetworkGroup {
        IbvNetworkGroup {
            rank_ids: self
                .all_group
                .rank_ids
                .iter()
                .cloned()
                .filter(filter)
                .collect(),
        }
    }

    pub fn self_group(
        &self,
        filter: impl FnMut(&usize) -> bool,
    ) -> Result<IbvNetworkSelfGroup, IbvNetworkGroup> {
        let rank_ids: Vec<_> = self
            .all_group
            .rank_ids
            .iter()
            .cloned()
            .filter(filter)
            .collect();
        let self_idx = rank_ids.binary_search(&self.rank_id());
        match self_idx {
            Ok(self_idx) => Ok(IbvNetworkSelfGroup { self_idx, rank_ids }),
            Err(_) => Err(IbvNetworkGroup { rank_ids }),
        }
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

impl<'network>
    RdmaNetworkSelfGroupConnections<'network, IbvMemoryRegion, IbvRemoteMemoryRegion, IbvConnection>
    for IbvNetworkSelfGroupConnections<'network>
{
    fn connection_mut(
        &mut self,
        idx: usize,
    ) -> Option<RdmaNetworkSelfGroupConnection<'_, IbvConnection>> {
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
