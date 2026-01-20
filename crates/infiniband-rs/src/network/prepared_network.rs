use crate::connection::prepared_connection::IbvPreparedConnection;
use crate::ibverbs::queue_pair_endpoint::IbvQueuePairEndpoint;
use crate::network::host::IbvNetworkRank;
use serde::{Deserialize, Serialize};

pub struct IbvPreparedNetwork {
    rank: IbvNetworkRank,
    connections: Vec<IbvPreparedConnection>,
}

impl IbvPreparedNetwork {
    pub(super) fn new(rank: IbvNetworkRank, connections: Vec<IbvPreparedConnection>) -> Self {
        Self { rank, connections }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IbvNetworkNodeEndpoint {
    rank: IbvNetworkRank,
    connection_endpoints: Vec<IbvQueuePairEndpoint>,
}

impl IbvPreparedNetwork {
    pub fn endpoint(&self) -> IbvNetworkNodeEndpoint {
        IbvNetworkNodeEndpoint {
            rank: self.rank,
            connection_endpoints: self
                .connections
                .iter()
                .map(|connection| connection.endpoint())
                .collect(),
        }
    }
}
