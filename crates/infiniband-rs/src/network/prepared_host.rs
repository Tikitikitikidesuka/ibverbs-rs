use crate::connection::prepared_connection::IbvPreparedConnection;
use crate::ibverbs::queue_pair_endpoint::IbvQueuePairEndpoint;
use crate::network::NodeError;
use crate::network::node::{Node, Rank};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub struct PreparedNode {
    rank: Rank,
    connections: Vec<IbvPreparedConnection>,
}

impl PreparedNode {
    pub(super) fn new(rank: Rank, connections: Vec<IbvPreparedConnection>) -> Self {
        Self { rank, connections }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeScatterEndpoint {
    rank: Rank,
    remote_endpoints: Vec<IbvQueuePairEndpoint>,
}

#[derive(Debug, Clone)]
pub struct NodeGatherEndpoint {
    rank: Rank,
    remote_endpoints: Vec<IbvQueuePairEndpoint>,
}

#[derive(Debug, Error)]
#[error("Unable to gather remote endpoints: host {remote_rank} did not provide endpoint {rank}")]
pub struct NodeEndpointGatherError {
    rank: Rank,
    remote_rank: Rank,
}

impl NodeGatherEndpoint {
    /// Takes a vector of network node endpoints and generates a new one
    /// containing a connection for each of the gathered nodes
    pub fn gather(
        rank: Rank,
        endpoints: impl IntoIterator<Item =NodeScatterEndpoint>,
    ) -> Result<NodeGatherEndpoint, NodeEndpointGatherError> {
        let remote_endpoints = endpoints
            .into_iter()
            .enumerate()
            .map(|(remote_rank, remote_endpoint)| {
                remote_endpoint
                    .remote_endpoints
                    .get(rank)
                    .ok_or(NodeEndpointGatherError { rank, remote_rank })
                    .cloned()
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(NodeGatherEndpoint {
            rank,
            remote_endpoints,
        })
    }
}

impl PreparedNode {
    pub fn endpoint(&self) -> NodeScatterEndpoint {
        NodeScatterEndpoint {
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
        endpoint: NodeGatherEndpoint,
    ) -> Result<Node, NodeError> {
        if endpoint.rank != self.rank {
            return Err(NodeError::RankMismatch {
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

        Ok(Node::new(self.rank, connections))
    }
}
