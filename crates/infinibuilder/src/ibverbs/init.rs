use crate::barrier::{RdmaNetworkBarrier, RdmaNetworkMemoryRegionComponent};
use crate::ibverbs::connection::{IbvMemoryRegion, IbvRemoteMemoryRegion};
use crate::ibverbs::network_node::{
    IbvNetworkNode, IbvNetworkNodeBuildError, IbvNetworkNodeBuilder, IbvNetworkNodeEndpoint,
    IbvNetworkNodeEndpointGatherError,
};
use crate::network_config::{NetworkConfigError, RawNetworkConfig};
use crate::tcp_exchanger::{TcpExchangeConfig, TcpExchanger, TcpNetworkConfigExchangeError};
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

pub fn create_ibv_network_node<NB, UNB>(
    rank_id: usize,
    cq_capacity: usize,
    cq_cache_capacity: usize,
    network_config: RawNetworkConfig,
    mrs: impl IntoIterator<Item = (impl Into<String>, *mut u8, usize)>,
    barrier: UNB,
) -> Result<IbvNetworkNode<NB>, IbvNetworkNodeInitError>
where
    UNB: RdmaNetworkMemoryRegionComponent<IbvMemoryRegion, IbvRemoteMemoryRegion, Registered = NB>,
    NB: RdmaNetworkBarrier<IbvMemoryRegion, IbvRemoteMemoryRegion>,
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
