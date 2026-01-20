use crate::connection::prepared_connection::IbvPreparedConnection;
use crate::ibverbs::queue_pair_endpoint::IbvQueuePairEndpoint;
use crate::network::IbvNetworkHostError;
use crate::network::host::{IbvNetworkHost, IbvNetworkRank};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub struct IbvPreparedNetworkHost {
    rank: IbvNetworkRank,
    connections: Vec<IbvPreparedConnection>,
}

impl IbvPreparedNetworkHost {
    pub(super) fn new(rank: IbvNetworkRank, connections: Vec<IbvPreparedConnection>) -> Self {
        Self { rank, connections }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IbvNetworkHostScatterEndpoint {
    rank: IbvNetworkRank,
    remote_endpoints: Vec<IbvQueuePairEndpoint>,
}

#[derive(Debug, Clone)]
pub struct IbvNetworkHostGatherEndpoint {
    rank: IbvNetworkRank,
    remote_endpoints: Vec<IbvQueuePairEndpoint>,
}

#[derive(Debug, Error)]
#[error("Unable to gather remote endpoints: host {remote_rank} did not provide endpoint {rank}")]
pub struct IbvNetworkHostEndpointGatherError {
    rank: IbvNetworkRank,
    remote_rank: IbvNetworkRank,
}

impl IbvNetworkHostGatherEndpoint {
    /// Takes a vector of network node endpoints and generates a new one
    /// containing a connection for each of the gathered nodes
    pub fn gather(
        rank: IbvNetworkRank,
        endpoints: impl IntoIterator<Item = IbvNetworkHostScatterEndpoint>,
    ) -> Result<IbvNetworkHostGatherEndpoint, IbvNetworkHostEndpointGatherError> {
        let remote_endpoints = endpoints
            .into_iter()
            .enumerate()
            .map(|(remote_rank, remote_endpoint)| {
                remote_endpoint
                    .remote_endpoints
                    .get(rank)
                    .ok_or(IbvNetworkHostEndpointGatherError { rank, remote_rank })
                    .cloned()
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(IbvNetworkHostGatherEndpoint {
            rank,
            remote_endpoints,
        })
    }
}

impl IbvPreparedNetworkHost {
    pub fn endpoint(&self) -> IbvNetworkHostScatterEndpoint {
        IbvNetworkHostScatterEndpoint {
            rank: self.rank,
            remote_endpoints: self
                .connections
                .iter()
                .map(|connection| connection.endpoint())
                .collect(),
        }
    }

    pub fn handshake(
        self,
        endpoint: IbvNetworkHostGatherEndpoint,
    ) -> Result<IbvNetworkHost, IbvNetworkHostError> {
        if endpoint.rank != self.rank {
            return Err(IbvNetworkHostError::RankMismatch {
                rank: endpoint.rank,
                expected: self.rank,
            });
        }

        let connections = self
            .connections
            .into_iter()
            .zip(endpoint.remote_endpoints)
            .map(|(connection, remote_endpoint)| connection.handshake(remote_endpoint))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(IbvNetworkHost::new(self.rank, connections))
    }
}
