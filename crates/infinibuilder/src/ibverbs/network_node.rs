use crate::barrier::RdmaNetworkNodeBarrier;
use crate::ibverbs::Named;
use crate::ibverbs::connection::{
    IbvConnection, IbvConnectionBuildError, IbvConnectionBuilder, IbvConnectionEndpoint,
    IbvMemoryRegion, IbvPreparedConnection, IbvRemoteMemoryRegion,
};
use crate::ibverbs::work_request::IbvWorkRequest;
use crate::rdma_connection::{
    RdmaNamedMemoryRegionConnection, RdmaNamedRemoteMemoryRegionConnection,
};
use crate::rdma_network_node::{
    MemoryRegionPair, RdmaBarrierNetworkNode, RdmaGroupNetworkNode, RdmaMemoryRegionNetworkNode,
    RdmaNamedMemory, RdmaNamedMemoryRegionNetworkNode, RdmaNamedRemoteMemoryRegionNetworkNode,
    RdmaNetworkGroup, RdmaNetworkMemoryRegionComponent, RdmaNetworkSelfGroup,
    RdmaNetworkSelfGroupConnection, RdmaNetworkSelfGroupConnections, RdmaRankIdNetworkNode,
    RdmaReadTransportNetworkNode, RdmaReceiveImmediateDataTransportNetworkNode, RdmaReceiveParams,
    RdmaReceiveTransportNetworkNode, RdmaRemoteMemoryRegionNetworkNode,
    RdmaSendImmediateDataTransportNetworkNode, RdmaSendParams, RdmaSendTransportNetworkNode,
    RdmaWriteTransportNetworkNode,
};
use crate::transport::{
    RdmaNetworkNodeReadTransport, RdmaNetworkNodeReceiveImmediateDataTransport,
    RdmaNetworkNodeReceiveTransport, RdmaNetworkNodeSendImmediateDataTransport,
    RdmaNetworkNodeSendTransport, RdmaNetworkNodeTransport, RdmaNetworkNodeWriteTransport,
};
use derivative::Derivative;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::collections::HashMap;
use std::error::Error;
use std::ops::RangeBounds;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

const BARRIER_MEM_ID: &str = "BARRIER_Xg9rgXPUXZ";
const TRANSPORT_META_MEM_ID: &str = "TRANSPORT_Htnjlt2vgk";

#[derive(Debug, Error)]
pub enum IbvNetworkNodeBuildError {
    #[error("Connection builder error: {0}")]
    ConnectionBuilderError(#[from] IbvConnectionBuildError),
    #[error("Barrier component memory register error: {0}")]
    BarrierMemoryRegisterError(String),
    #[error("Transport memory register error: {0}")]
    TransportMemoryRegisterError(String),
}

pub struct IbvNetworkNodeBuilder<RankId, IbvDevName, CqParams, NumConns, Barrier, Transport> {
    rank_id: RankId,
    ibv_device_name: IbvDevName,
    completion_queue_params: CqParams,
    num_connections: NumConns,
    barrier: Barrier,
    transport: Transport,
    transport_mrs: Vec<RdmaNamedMemory>,
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
    has_mrs: bool,
}

#[derive(Debug)]
pub struct BuilderTransport<PreparedTransport> {
    transport: PreparedTransport,
    has_mrs: bool,
}

#[derive(Debug)]
pub struct BuilderTransportMrs {
    mrs: Vec<(String, *mut u8, usize)>,
}

impl IbvNetworkNodeBuilder<(), (), (), (), (), ()> {
    pub fn new() -> Self {
        Self {
            rank_id: (),
            ibv_device_name: (),
            completion_queue_params: (),
            num_connections: (),
            barrier: (),
            transport: (),
            transport_mrs: vec![],
        }
    }
}

impl<IbvDevName, CqParams, NumConns, PreparedBarrier, PreparedTransport>
    IbvNetworkNodeBuilder<(), IbvDevName, CqParams, NumConns, PreparedBarrier, PreparedTransport>
{
    pub fn rank_id(
        self,
        rank_id: usize,
    ) -> IbvNetworkNodeBuilder<
        BuilderRankId,
        IbvDevName,
        CqParams,
        NumConns,
        PreparedBarrier,
        PreparedTransport,
    > {
        IbvNetworkNodeBuilder {
            rank_id: BuilderRankId { rank_id },
            ibv_device_name: self.ibv_device_name,
            completion_queue_params: self.completion_queue_params,
            num_connections: self.num_connections,
            barrier: self.barrier,
            transport: self.transport,
            transport_mrs: self.transport_mrs,
        }
    }
}

impl<RankId, CqParams, NumConns, PreparedBarrier, PreparedTransport>
    IbvNetworkNodeBuilder<RankId, (), CqParams, NumConns, PreparedBarrier, PreparedTransport>
{
    pub fn ibv_device(
        self,
        device_name: impl Into<String>,
    ) -> IbvNetworkNodeBuilder<
        RankId,
        BuilderIbvDeviceName,
        CqParams,
        NumConns,
        PreparedBarrier,
        PreparedTransport,
    > {
        IbvNetworkNodeBuilder {
            rank_id: self.rank_id,
            ibv_device_name: BuilderIbvDeviceName {
                ibv_device_name: device_name.into(),
            },
            completion_queue_params: self.completion_queue_params,
            num_connections: self.num_connections,
            barrier: self.barrier,
            transport: self.transport,
            transport_mrs: self.transport_mrs,
        }
    }
}

impl<RankId, IbvDevName, NumConns, PreparedBarrier, PreparedTransport>
    IbvNetworkNodeBuilder<RankId, IbvDevName, (), NumConns, PreparedBarrier, PreparedTransport>
{
    pub fn cq_params(
        self,
        capacity: usize,
        cache_capacity: usize,
    ) -> IbvNetworkNodeBuilder<
        RankId,
        IbvDevName,
        BuilderCqParams,
        NumConns,
        PreparedBarrier,
        PreparedTransport,
    > {
        IbvNetworkNodeBuilder {
            rank_id: self.rank_id,
            ibv_device_name: self.ibv_device_name,
            completion_queue_params: BuilderCqParams {
                capacity,
                cache_capacity,
            },
            num_connections: self.num_connections,
            barrier: self.barrier,
            transport: self.transport,
            transport_mrs: self.transport_mrs,
        }
    }
}

impl<RankId, IbvDevName, CqParams, PreparedBarrier, PreparedTransport>
    IbvNetworkNodeBuilder<RankId, IbvDevName, CqParams, (), PreparedBarrier, PreparedTransport>
{
    pub fn num_connections(
        self,
        num_connections: usize,
    ) -> IbvNetworkNodeBuilder<
        RankId,
        IbvDevName,
        CqParams,
        BuilderNumConnections,
        PreparedBarrier,
        PreparedTransport,
    > {
        IbvNetworkNodeBuilder {
            rank_id: self.rank_id,
            ibv_device_name: self.ibv_device_name,
            completion_queue_params: self.completion_queue_params,
            num_connections: BuilderNumConnections { num_connections },
            barrier: self.barrier,
            transport: self.transport,
            transport_mrs: self.transport_mrs,
        }
    }
}

impl<RankId, IbvDevName, CqParams, NumConnections, PreparedTransport>
    IbvNetworkNodeBuilder<RankId, IbvDevName, CqParams, NumConnections, (), PreparedTransport>
{
    pub fn barrier<
        Barrier: RdmaNetworkNodeBarrier<IbvConnection>,
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
        PreparedTransport,
    > {
        IbvNetworkNodeBuilder {
            rank_id: self.rank_id,
            ibv_device_name: self.ibv_device_name,
            completion_queue_params: self.completion_queue_params,
            num_connections: self.num_connections,
            barrier: BuilderBarrier {
                barrier,
                has_mrs: false,
            },
            transport: self.transport,
            transport_mrs: self.transport_mrs,
        }
    }
}

impl<RankId, IbvDevName, CqParams, NumConnections, PreparedBarrier>
    IbvNetworkNodeBuilder<RankId, IbvDevName, CqParams, NumConnections, PreparedBarrier, ()>
{
    pub fn transport<
        Transport: RdmaNetworkNodeTransport<IbvConnection>,
        PreparedTransport: RdmaNetworkMemoryRegionComponent<
                IbvMemoryRegion,
                IbvRemoteMemoryRegion,
                Registered = Transport,
            >,
    >(
        self,
        transport: PreparedTransport,
    ) -> IbvNetworkNodeBuilder<
        RankId,
        IbvDevName,
        CqParams,
        NumConnections,
        PreparedBarrier,
        BuilderTransport<PreparedTransport>,
    > {
        IbvNetworkNodeBuilder {
            rank_id: self.rank_id,
            ibv_device_name: self.ibv_device_name,
            completion_queue_params: self.completion_queue_params,
            num_connections: self.num_connections,
            barrier: self.barrier,
            transport: BuilderTransport {
                transport,
                has_mrs: false,
            },
            transport_mrs: self.transport_mrs,
        }
    }
}

impl<RankId, IbvDevName, CqParams, NumConnections, PreparedBarrier, PreparedTransport>
    IbvNetworkNodeBuilder<
        RankId,
        IbvDevName,
        CqParams,
        NumConnections,
        PreparedBarrier,
        PreparedTransport,
    >
{
    pub fn register_mr(
        mut self,
        memory: RdmaNamedMemory,
    ) -> IbvNetworkNodeBuilder<
        RankId,
        IbvDevName,
        CqParams,
        NumConnections,
        PreparedBarrier,
        PreparedTransport,
    > {
        self.transport_mrs.push(memory);

        IbvNetworkNodeBuilder {
            rank_id: self.rank_id,
            ibv_device_name: self.ibv_device_name,
            completion_queue_params: self.completion_queue_params,
            num_connections: self.num_connections,
            barrier: self.barrier,
            transport: self.transport,
            transport_mrs: self.transport_mrs,
        }
    }

    pub fn register_mrs(
        mut self,
        mrs: impl IntoIterator<Item = RdmaNamedMemory>,
    ) -> IbvNetworkNodeBuilder<
        RankId,
        IbvDevName,
        CqParams,
        NumConnections,
        PreparedBarrier,
        PreparedTransport,
    > {
        self.transport_mrs
            .append(&mut mrs.into_iter().collect::<Vec<_>>());

        IbvNetworkNodeBuilder {
            rank_id: self.rank_id,
            ibv_device_name: self.ibv_device_name,
            completion_queue_params: self.completion_queue_params,
            num_connections: self.num_connections,
            barrier: self.barrier,
            transport: self.transport,
            transport_mrs: self.transport_mrs,
        }
    }
}

impl<
    Barrier: RdmaNetworkNodeBarrier<IbvConnection>,
    PreparedBarrier: RdmaNetworkMemoryRegionComponent<IbvMemoryRegion, IbvRemoteMemoryRegion, Registered = Barrier>,
    Transport: RdmaNetworkNodeTransport<IbvConnection>,
    PreparedTransport: RdmaNetworkMemoryRegionComponent<
            IbvMemoryRegion,
            IbvRemoteMemoryRegion,
            Registered = Transport,
        >,
>
    IbvNetworkNodeBuilder<
        BuilderRankId,
        BuilderIbvDeviceName,
        BuilderCqParams,
        BuilderNumConnections,
        BuilderBarrier<PreparedBarrier>,
        BuilderTransport<PreparedTransport>,
    >
{
    pub fn build(
        mut self,
    ) -> Result<
        IbvPreparedNetworkNode<Barrier, PreparedBarrier, Transport, PreparedTransport>,
        IbvNetworkNodeBuildError,
    > {
        // Create connections with device and cq config
        let connection_builder = IbvConnectionBuilder::new()
            .ibv_device(self.ibv_device_name.ibv_device_name)
            .cq_params(
                self.completion_queue_params.capacity,
                self.completion_queue_params.cache_capacity,
            );

        // Clone connection builder for each connection
        let connection_builders = (0..self.num_connections.num_connections)
            .map(|_| connection_builder.clone().lock_clone())
            .collect::<Vec<_>>();

        // Get memory regions from the barrier
        let connection_builders = if let Some(barrier_mem_vec) = self
            .barrier
            .barrier
            .memory(self.num_connections.num_connections)
        {
            // To later register or not
            self.barrier.has_mrs = true;

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
            connection_builders
                .into_iter()
                .zip(barrier_mem_vec)
                .map(|(conn_builder, (mem_ptr, mem_length))| {
                    conn_builder.register_mr(RdmaNamedMemory::new(
                        BARRIER_MEM_ID, // To mark it as the barrier mem of this connection
                        mem_ptr,
                        mem_length,
                    ))
                })
                .collect()
        } else {
            // To later register or not
            self.barrier.has_mrs = false;

            connection_builders
        };

        // Get memory regions from the barrier
        let connection_builders = if let Some(transport_meta_mem_vec) = self
            .transport
            .transport
            .memory(self.num_connections.num_connections)
        {
            // To later register or not
            self.transport.has_mrs = true;

            // Check the number of memory region matches the number of connections
            if transport_meta_mem_vec.len() != self.num_connections.num_connections {
                return Err(IbvNetworkNodeBuildError::BarrierMemoryRegisterError(
                    format!(
                        "Non matching memory region number: {} regions returned by the transport component for {} connections",
                        transport_meta_mem_vec.len(),
                        self.num_connections.num_connections
                    ),
                ));
            }

            // Register the barrier memory regions in their corresponding connections
            connection_builders
                .into_iter()
                .zip(transport_meta_mem_vec)
                .map(|(conn_builder, (mem_ptr, mem_length))| {
                    conn_builder.register_mr(RdmaNamedMemory::new(
                        TRANSPORT_META_MEM_ID, // To mark it as the transport meta mem of this connection
                        mem_ptr,
                        mem_length,
                    ))
                })
                .collect()
        } else {
            // To later register or not
            self.transport.has_mrs = false;

            connection_builders
        };

        // Register the transport memory regions on every connection
        let connection_builders = connection_builders
            .into_iter()
            .map(|conn_builder| conn_builder.register_mrs(self.transport_mrs.clone()))
            .collect::<Vec<_>>();

        // Build all connections
        let prepared_connections = connection_builders
            .into_iter()
            .map(|conn_builder| conn_builder.build())
            .collect::<Result<Vec<_>, _>>()?;

        Ok(IbvPreparedNetworkNode {
            rank_id: self.rank_id.rank_id,
            prepared_connections,
            prepared_barrier: self.barrier.barrier,
            barrier_has_mrs: self.barrier.has_mrs,
            prepared_transport: self.transport.transport,
            transport_has_mrs: self.transport.has_mrs,
            transport_mrs: self.transport_mrs,
        })
    }
}

pub struct IbvPreparedNetworkNode<
    Barrier: RdmaNetworkNodeBarrier<IbvConnection>,
    PreparedBarrier: RdmaNetworkMemoryRegionComponent<IbvMemoryRegion, IbvRemoteMemoryRegion, Registered = Barrier>,
    Transport: RdmaNetworkNodeTransport<IbvConnection>,
    PreparedTransport: RdmaNetworkMemoryRegionComponent<
            IbvMemoryRegion,
            IbvRemoteMemoryRegion,
            Registered = Transport,
        >,
> {
    rank_id: usize,
    prepared_connections: Vec<IbvPreparedConnection>,
    prepared_barrier: PreparedBarrier,
    barrier_has_mrs: bool,
    prepared_transport: PreparedTransport,
    transport_has_mrs: bool,
    transport_mrs: Vec<RdmaNamedMemory>,
}

impl<
    Barrier: RdmaNetworkNodeBarrier<IbvConnection>,
    PreparedBarrier: RdmaNetworkMemoryRegionComponent<IbvMemoryRegion, IbvRemoteMemoryRegion, Registered = Barrier>,
    Transport: RdmaNetworkNodeTransport<IbvConnection>,
    PreparedTransport: RdmaNetworkMemoryRegionComponent<
            IbvMemoryRegion,
            IbvRemoteMemoryRegion,
            Registered = Transport,
        >,
> IbvPreparedNetworkNode<Barrier, PreparedBarrier, Transport, PreparedTransport>
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
    ) -> Result<IbvNetworkNode<Barrier, Transport>, IbvNetworkNodeBuildError> {
        let greatest_rank_id = self.prepared_connections.len().saturating_sub(1);

        let connections = self
            .prepared_connections
            .into_iter()
            .zip(connection_config.connection_endpoints)
            .map(|(prepared_connection, endpoint)| prepared_connection.connect(endpoint))
            .collect::<Result<Vec<_>, _>>()?;

        let barrier = if self.barrier_has_mrs {
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
                .registered_mrs(Some(barrier_mrs))
                .map_err(|error| {
                    IbvNetworkNodeBuildError::BarrierMemoryRegisterError(format!("{error}"))
                })?;

            barrier
        } else {
            let barrier = self
                .prepared_barrier
                .registered_mrs(None)
                .map_err(|error| {
                    IbvNetworkNodeBuildError::BarrierMemoryRegisterError(format!("{error}"))
                })?;

            barrier
        };

        let transport = if self.transport_has_mrs {
            let transport_meta_mrs = connections
                .iter()
                .enumerate()
                .map(|(idx, conn)| {
                    Ok(MemoryRegionPair {
                        local_mr: conn.local_mr(TRANSPORT_META_MEM_ID).ok_or(
                            IbvNetworkNodeBuildError::BarrierMemoryRegisterError(format!(
                                "Missing transport meta local memory for conn {idx}"
                            )),
                        )?,
                        remote_mr: conn.remote_mr(TRANSPORT_META_MEM_ID).ok_or(
                            IbvNetworkNodeBuildError::BarrierMemoryRegisterError(format!(
                                "Missing transport meta remote memory for conn {idx}"
                            )),
                        )?,
                    })
                })
                .collect::<Result<Vec<_>, IbvNetworkNodeBuildError>>()?;

            let transport = self
                .prepared_transport
                .registered_mrs(Some(transport_meta_mrs))
                .map_err(|error| {
                    IbvNetworkNodeBuildError::BarrierMemoryRegisterError(format!("{error}"))
                })?;

            transport
        } else {
            let transport = self
                .prepared_transport
                .registered_mrs(None)
                .map_err(|error| {
                    IbvNetworkNodeBuildError::BarrierMemoryRegisterError(format!("{error}"))
                })?;

            transport
        };

        let transport_mrs = self
            .transport_mrs
            .iter()
            .enumerate()
            .map(|(idx, memory)| {
                // Mrs named `mr_id` of each connection
                let local_mrs: Vec<_> = connections
                    .iter()
                    .map(|conn| {
                        conn.local_mr(&memory.id).ok_or(
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
                        conn.remote_mr(&memory.id).ok_or(
                            IbvNetworkNodeBuildError::TransportMemoryRegisterError(format!(
                                "Missing transport remote memory for conn {idx}"
                            )),
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                Ok((
                    memory.id.clone(),
                    MemoryRegionPair {
                        local_mr: IbvNetworkNodeMemoryRegion {
                            conn_mrs: Arc::new(Named::new(memory.id.clone(), local_mrs)),
                        },
                        remote_mr: IbvNetworkNodeRemoteMemoryRegion {
                            conn_mrs: Arc::new(Named::new(memory.id.clone(), remote_mrs)),
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
            transport,
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

    pub fn pair(
        primary: bool,
        own_endpoint: &IbvNetworkNodeEndpoint,
        other_endpoint: &IbvNetworkNodeEndpoint,
    ) -> IbvNetworkNodeEndpoint {
        assert_eq!(own_endpoint.connection_endpoints.len(), 2);
        assert_eq!(other_endpoint.connection_endpoints.len(), 2);
        let mut endpoints = vec![
            own_endpoint.connection_endpoints[primary as usize].clone(),
            other_endpoint.connection_endpoints[primary as usize].clone(),
        ];
        if primary {
            endpoints.reverse();
        }
        IbvNetworkNodeEndpoint {
            rank_id: primary as _,
            connection_endpoints: endpoints,
        }
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct IbvNetworkNode<NB, NT> {
    rank_id: usize,
    greatest_rank_id: usize,
    all_group: IbvNetworkSelfGroup,
    connections: Vec<IbvConnection>,
    barrier: NB,
    transport: NT,
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

impl<NB, NT> RdmaRankIdNetworkNode for IbvNetworkNode<NB, NT> {
    fn rank_id(&self) -> usize {
        self.rank_id
    }
}

impl<NB, NT> RdmaMemoryRegionNetworkNode for IbvNetworkNode<NB, NT> {
    type MemoryRegion = IbvNetworkNodeMemoryRegion;
}

impl<NB, NT> RdmaRemoteMemoryRegionNetworkNode for IbvNetworkNode<NB, NT> {
    type RemoteMemoryRegion = IbvNetworkNodeRemoteMemoryRegion;
}

impl<NB, NT> RdmaNamedMemoryRegionNetworkNode for IbvNetworkNode<NB, NT> {
    fn local_mr(&self, id: impl AsRef<str>) -> Option<IbvNetworkNodeMemoryRegion> {
        self.transport_mrs
            .get(id.as_ref())
            .map(|mr_pair| mr_pair.local_mr.clone())
    }
}

impl<NB, NT> RdmaNamedRemoteMemoryRegionNetworkNode for IbvNetworkNode<NB, NT> {
    fn remote_mr(&self, id: impl AsRef<str>) -> Option<IbvNetworkNodeRemoteMemoryRegion> {
        self.transport_mrs
            .get(id.as_ref())
            .map(|mr_pair| mr_pair.remote_mr.clone())
    }
}

impl<NB, NT> RdmaGroupNetworkNode for IbvNetworkNode<NB, NT> {
    type Group = IbvNetworkGroup;
    type SelfGroup = IbvNetworkSelfGroup;

    fn group_all(&self) -> Self::SelfGroup {
        self.all_group.clone()
    }

    fn group_peers(&self) -> Self::Group {
        todo!()
    }
}

impl<NB: RdmaNetworkNodeBarrier<IbvConnection>, NT> RdmaBarrierNetworkNode
    for IbvNetworkNode<NB, NT>
{
    type BarrierError = NB::Error;

    fn barrier<Group>(&mut self, group: &Group, timeout: Duration) -> Result<(), Self::BarrierError>
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
    RdmaPostError(PostError),
}

impl<NB, NT: RdmaNetworkNodeSendTransport<IbvConnection>> RdmaSendTransportNetworkNode
    for IbvNetworkNode<NB, NT>
{
    type WorkRequest = NT::WorkRequest;
    type PostError = ConnectionTransportPostError<NT::PostError>;

    fn post_send(
        &mut self,
        peer_rank_id: usize,
        memory_region: &Self::MemoryRegion,
        memory_range: impl RangeBounds<usize> + Clone,
        immediate_data: Option<u32>,
    ) -> Result<Self::WorkRequest, Self::PostError> {
        if peer_rank_id == self.all_group.self_rank_id() {
            return Err(ConnectionTransportPostError::SelfPeerRankId);
        }

        if peer_rank_id > self.greatest_rank_id {
            return Err(ConnectionTransportPostError::InvalidPeerRankId(
                peer_rank_id,
            ));
        }

        Ok(self
            .transport
            .post_send(
                peer_rank_id,
                &mut self.connections[peer_rank_id],
                &memory_region.conn_mrs.data[peer_rank_id],
                memory_range,
                immediate_data,
            )
            .map_err(|e| ConnectionTransportPostError::RdmaPostError(e))?)
    }

    fn post_send_batch<'a, Range: RangeBounds<usize> + Clone>(
        &mut self,
        peer_rank_id: usize,
        send_params_iter: impl IntoIterator<
            Item = impl Borrow<RdmaSendParams<'a, Self::MemoryRegion, Range>>,
        >,
    ) -> Vec<Result<Self::WorkRequest, Self::PostError>>
    where
        <Self as RdmaMemoryRegionNetworkNode>::MemoryRegion: 'a,
    {
        if peer_rank_id == self.all_group.self_rank_id() {
            return send_params_iter
                .into_iter()
                .map(|_| Err(ConnectionTransportPostError::SelfPeerRankId))
                .collect();
        }

        if peer_rank_id > self.greatest_rank_id {
            return send_params_iter
                .into_iter()
                .map(|_| {
                    Err(ConnectionTransportPostError::InvalidPeerRankId(
                        peer_rank_id,
                    ))
                })
                .collect();
        }

        self.transport
            .post_send_batch(
                peer_rank_id,
                &mut self.connections[peer_rank_id],
                send_params_iter
                    .into_iter()
                    .map(|send_params| RdmaSendParams {
                        memory_region: &send_params.borrow().memory_region.conn_mrs.data
                            [peer_rank_id],
                        memory_range: send_params.borrow().memory_range.clone(),
                        immediate_data: send_params.borrow().immediate_data.clone(),
                    }),
            )
            .into_iter()
            .map(|result| result.map_err(|e| ConnectionTransportPostError::RdmaPostError(e)))
            .collect()
    }
}

impl<NB, NT: RdmaNetworkNodeReceiveTransport<IbvConnection>> RdmaReceiveTransportNetworkNode
    for IbvNetworkNode<NB, NT>
{
    type WorkRequest = NT::WorkRequest;
    type PostError = ConnectionTransportPostError<NT::PostError>;

    fn post_receive(
        &mut self,
        peer_rank_id: usize,
        memory_region: &IbvNetworkNodeMemoryRegion,
        memory_range: impl RangeBounds<usize> + Clone,
    ) -> Result<Self::WorkRequest, Self::PostError> {
        if peer_rank_id == self.all_group.self_rank_id() {
            return Err(ConnectionTransportPostError::SelfPeerRankId);
        }

        if peer_rank_id > self.greatest_rank_id {
            return Err(ConnectionTransportPostError::InvalidPeerRankId(
                peer_rank_id,
            ));
        }

        Ok(self
            .transport
            .post_receive(
                peer_rank_id,
                &mut self.connections[peer_rank_id],
                &memory_region.conn_mrs.data[peer_rank_id],
                memory_range,
            )
            .map_err(|e| ConnectionTransportPostError::RdmaPostError(e))?)
    }

    fn post_receive_batch<'a, Range: RangeBounds<usize> + Clone>(
        &mut self,
        peer_rank_id: usize,
        receive_params_iter: impl IntoIterator<
            Item = impl Borrow<RdmaReceiveParams<'a, Self::MemoryRegion, Range>>,
        >,
    ) -> Vec<Result<Self::WorkRequest, Self::PostError>>
    where
        <Self as RdmaMemoryRegionNetworkNode>::MemoryRegion: 'a,
    {
        if peer_rank_id == self.all_group.self_rank_id() {
            return receive_params_iter
                .into_iter()
                .map(|_| Err(ConnectionTransportPostError::SelfPeerRankId))
                .collect();
        }

        if peer_rank_id > self.greatest_rank_id {
            return receive_params_iter
                .into_iter()
                .map(|_| {
                    Err(ConnectionTransportPostError::InvalidPeerRankId(
                        peer_rank_id,
                    ))
                })
                .collect();
        }

        self.transport
            .post_receive_batch(
                peer_rank_id,
                &mut self.connections[peer_rank_id],
                receive_params_iter
                    .into_iter()
                    .map(|receive_params| RdmaReceiveParams {
                        memory_region: &receive_params.borrow().memory_region.conn_mrs.data
                            [peer_rank_id],
                        memory_range: receive_params.borrow().memory_range.clone(),
                    }),
            )
            .into_iter()
            .map(|result| result.map_err(|e| ConnectionTransportPostError::RdmaPostError(e)))
            .collect()
    }
}

impl<NB, NT: RdmaNetworkNodeWriteTransport<IbvConnection>> RdmaWriteTransportNetworkNode
    for IbvNetworkNode<NB, NT>
{
    type WorkRequest = NT::WorkRequest;
    type PostError = ConnectionTransportPostError<NT::PostError>;

    fn post_write(
        &mut self,
        peer_rank_id: usize,
        local_memory_region: &IbvNetworkNodeMemoryRegion,
        local_memory_range: impl RangeBounds<usize> + Clone,
        remote_memory_region: &IbvNetworkNodeRemoteMemoryRegion,
        remote_memory_range: impl RangeBounds<usize> + Clone,
        immediate_data: Option<u32>,
    ) -> Result<Self::WorkRequest, Self::PostError> {
        if peer_rank_id == self.all_group.self_rank_id() {
            return Err(ConnectionTransportPostError::SelfPeerRankId);
        }

        if peer_rank_id > self.greatest_rank_id {
            return Err(ConnectionTransportPostError::InvalidPeerRankId(
                peer_rank_id,
            ));
        }

        Ok(self
            .transport
            .post_write(
                peer_rank_id,
                &mut self.connections[peer_rank_id],
                &local_memory_region.conn_mrs.data[peer_rank_id],
                local_memory_range,
                &remote_memory_region.conn_mrs.data[peer_rank_id],
                remote_memory_range,
                immediate_data,
            )
            .map_err(|e| ConnectionTransportPostError::RdmaPostError(e))?)
    }
}

impl<NB, NT: RdmaNetworkNodeReadTransport<IbvConnection>> RdmaReadTransportNetworkNode
    for IbvNetworkNode<NB, NT>
{
    type WorkRequest = NT::WorkRequest;
    type PostError = ConnectionTransportPostError<NT::PostError>;

    fn post_read(
        &mut self,
        peer_rank_id: usize,
        local_memory_region: &IbvNetworkNodeMemoryRegion,
        local_memory_range: impl RangeBounds<usize> + Clone,
        remote_memory_region: &IbvNetworkNodeRemoteMemoryRegion,
        remote_memory_range: impl RangeBounds<usize> + Clone,
    ) -> Result<Self::WorkRequest, Self::PostError> {
        if peer_rank_id == self.all_group.self_rank_id() {
            return Err(ConnectionTransportPostError::SelfPeerRankId);
        }

        if peer_rank_id > self.greatest_rank_id {
            return Err(ConnectionTransportPostError::InvalidPeerRankId(
                peer_rank_id,
            ));
        }

        Ok(self
            .transport
            .post_read(
                peer_rank_id,
                &mut self.connections[peer_rank_id],
                &local_memory_region.conn_mrs.data[peer_rank_id],
                local_memory_range,
                &remote_memory_region.conn_mrs.data[peer_rank_id],
                remote_memory_range,
            )
            .map_err(|e| ConnectionTransportPostError::RdmaPostError(e))?)
    }
}

impl<NB, NT: RdmaNetworkNodeSendImmediateDataTransport<IbvConnection>>
    RdmaSendImmediateDataTransportNetworkNode for IbvNetworkNode<NB, NT>
{
    type WorkRequest = NT::WorkRequest;
    type PostError = ConnectionTransportPostError<NT::PostError>;

    fn post_send_immediate_data(
        &mut self,
        peer_rank_id: usize,
        immediate_data: u32,
    ) -> Result<Self::WorkRequest, Self::PostError> {
        if peer_rank_id == self.all_group.self_rank_id() {
            return Err(ConnectionTransportPostError::SelfPeerRankId);
        }

        if peer_rank_id > self.greatest_rank_id {
            return Err(ConnectionTransportPostError::InvalidPeerRankId(
                peer_rank_id,
            ));
        }

        Ok(self
            .transport
            .post_send_immediate_data(
                peer_rank_id,
                &mut self.connections[peer_rank_id],
                immediate_data,
            )
            .map_err(|e| ConnectionTransportPostError::RdmaPostError(e))?)
    }
}

impl<NB, NT: RdmaNetworkNodeReceiveImmediateDataTransport<IbvConnection>>
    RdmaReceiveImmediateDataTransportNetworkNode for IbvNetworkNode<NB, NT>
{
    type WorkRequest = NT::WorkRequest;
    type PostError = ConnectionTransportPostError<NT::PostError>;

    fn post_receive_immediate_data(
        &mut self,
        peer_rank_id: usize,
    ) -> Result<Self::WorkRequest, Self::PostError> {
        if peer_rank_id == self.all_group.self_rank_id() {
            return Err(ConnectionTransportPostError::SelfPeerRankId);
        }

        if peer_rank_id > self.greatest_rank_id {
            return Err(ConnectionTransportPostError::InvalidPeerRankId(
                peer_rank_id,
            ));
        }

        Ok(self
            .transport
            .post_receive_immediate_data(peer_rank_id, &mut self.connections[peer_rank_id])
            .map_err(|e| ConnectionTransportPostError::RdmaPostError(e))?)
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

impl<NB: RdmaNetworkNodeBarrier<IbvConnection>, NT> IbvNetworkNode<NB, NT> {
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

impl<'network> RdmaNetworkSelfGroupConnections<'network>
    for IbvNetworkSelfGroupConnections<'network>
{
    type Connection = IbvConnection;

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
