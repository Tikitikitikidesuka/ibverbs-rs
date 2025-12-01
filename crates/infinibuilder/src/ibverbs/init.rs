use crate::barrier::RdmaNetworkNodeBarrier;
use crate::ibverbs::connection::{IbvConnection, IbvMemoryRegion, IbvRemoteMemoryRegion};
use crate::ibverbs::network_node::{
    IbvNetworkNode, IbvNetworkNodeBuildError, IbvNetworkNodeBuilder, IbvNetworkNodeEndpoint,
    IbvNetworkNodeEndpointGatherError,
};
use crate::network_config::{NetworkConfigError, RawNetworkConfig};
use crate::rdma_network_node::{RdmaNamedMemory, RdmaNetworkMemoryRegionComponent};
use crate::tcp_exchanger::{TcpExchangeConfig, TcpExchanger, TcpNetworkConfigExchangeError};
use crate::transport::RdmaNetworkNodeTransport;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IbvNetworkNodeInitError {
    #[error("Network config error: {0}")]
    NetworkConfigError(#[from] NetworkConfigError),
    #[error("Rank id {0} not in network")]
    InvalidRankId(usize),
    #[error("Error building network node: {0}")]
    NetworkNodeBuildError(#[from] IbvNetworkNodeBuildError),
    #[error("Endpoint exchange error: {0}")]
    EndpointExchangeError(#[from] TcpNetworkConfigExchangeError),
    #[error("Endpoint gather error: {0}")]
    EndpointGatherError(#[from] IbvNetworkNodeEndpointGatherError),
}

pub fn create_ibv_network_node<NB, UNB, NT, UNT>(
    rank_id: usize,
    cq_capacity: usize,
    cq_cache_capacity: usize,
    network_config: RawNetworkConfig,
    mrs: impl IntoIterator<Item = RdmaNamedMemory>,
    barrier: UNB,
    transport: UNT,
) -> Result<IbvNetworkNode<NB, NT>, IbvNetworkNodeInitError>
where
    UNB: RdmaNetworkMemoryRegionComponent<IbvMemoryRegion, IbvRemoteMemoryRegion, Registered = NB>,
    NB: RdmaNetworkNodeBarrier<IbvConnection>,
    UNT: RdmaNetworkMemoryRegionComponent<IbvMemoryRegion, IbvRemoteMemoryRegion, Registered = NT>,
    NT: RdmaNetworkNodeTransport<IbvConnection>,
{
    let network_config = network_config.validate()?;

    let node_config = network_config
        .get(rank_id)
        .ok_or(IbvNetworkNodeInitError::InvalidRankId(rank_id))?;

    let prepared_node = IbvNetworkNodeBuilder::new()
        .ibv_device(&node_config.ibdev)
        .cq_params(cq_capacity, cq_cache_capacity)
        .barrier(barrier)
        .register_mrs(mrs)
        .num_connections(network_config.len())
        .rank_id(rank_id)
        .transport(transport)
        .build()?;

    let endpoint = prepared_node.endpoint();

    let exchanged_endpoints = TcpExchanger::await_exchange_all(
        rank_id,
        &network_config,
        &endpoint,
        &TcpExchangeConfig::default(),
    )?;

    let remote_endpoint = IbvNetworkNodeEndpoint::gather(rank_id, exchanged_endpoints)?;

    let node = prepared_node.connect(remote_endpoint)?;

    Ok(node)
}

/// Depending on whether `primary` is true or false, comm will be the socket to listen on or connect to, respectively.
pub fn create_ibv_pair_node<NB, UNB, NT, UNT>(
    primary: bool,
    addr: (&str, u16),
    ib_dev: impl Into<String>,
    cq_capacity: usize,
    cq_cache_capacity: usize,
    mrs: impl IntoIterator<Item = RdmaNamedMemory>,
    barrier: UNB,
    transport: UNT,
) -> Result<IbvNetworkNode<NB, NT>, IbvNetworkNodeInitError>
where
    UNB: RdmaNetworkMemoryRegionComponent<IbvMemoryRegion, IbvRemoteMemoryRegion, Registered = NB>,
    NB: RdmaNetworkNodeBarrier<IbvConnection>,
    UNT: RdmaNetworkMemoryRegionComponent<IbvMemoryRegion, IbvRemoteMemoryRegion, Registered = NT>,
    NT: RdmaNetworkNodeTransport<IbvConnection>,
{
    let rank = primary as usize;

    let prepared_node = IbvNetworkNodeBuilder::new()
        .ibv_device(ib_dev)
        .cq_params(cq_capacity, cq_cache_capacity)
        .barrier(barrier)
        .register_mrs(mrs)
        .num_connections(2)
        .rank_id(rank)
        .transport(transport)
        .build()?;

    let endpoint = prepared_node.endpoint();
    let exchanged_endpoint =
        TcpExchanger::await_exchange_pair(primary, addr, &endpoint, &TcpExchangeConfig::default())?;

    let mut endpoints = [endpoint, exchanged_endpoint];

    if primary {
        endpoints.reverse();
    }

    let remote_endpoint = IbvNetworkNodeEndpoint::gather(primary as _, endpoints)?;

    let node = prepared_node.connect(remote_endpoint)?;

    Ok(node)
}
