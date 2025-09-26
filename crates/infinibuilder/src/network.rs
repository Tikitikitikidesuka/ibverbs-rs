use crate::connect::Connect;
use crate::rdma_traits::{RdmaRendezvous, RdmaSendRecv};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub struct UnconnectedNetworkNode<Conn: Connect> {
    pub(crate) connections: Vec<Conn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkNodeConnectionConfig<ConnConf> {
    configs: Vec<ConnConf>,
}

pub struct ConnectedNetworkNode<T: RdmaSendRecv + RdmaRendezvous> {
    connections: Vec<T>,
}

impl<
    Conn: Connect<Connected = T, ConnectionConfig = ConnConf>,
    T: RdmaSendRecv + RdmaRendezvous,
    ConnConf,
> Connect for UnconnectedNetworkNode<Conn>
{
    type ConnectionConfig = NetworkNodeConnectionConfig<ConnConf>;
    type Connected = ConnectedNetworkNode<T>;

    fn connection_config(&self) -> Self::ConnectionConfig {
        NetworkNodeConnectionConfig {
            configs: self
                .connections
                .iter()
                .map(|c| c.connection_config())
                .collect(),
        }
    }

    fn connect(
        self,
        connection_config: Self::ConnectionConfig,
    ) -> std::io::Result<Self::Connected> {
        Ok(ConnectedNetworkNode {
            connections: self
                .connections
                .into_iter()
                .zip(connection_config.configs)
                .map(|(connection, config)| connection.connect(config))
                .collect::<Result<_, _>>()?,
        })
    }
}

#[derive(Debug, Error)]
pub enum ConnectionConfigGatherError {
    #[error("Not enough connections for gather from node with rank id {rank_id}")]
    NotEnoughConnectionsFromNode { rank_id: usize },
}

impl<ConnConf: Clone> NetworkNodeConnectionConfig<ConnConf> {
    pub fn gather(
        remote_configs: impl IntoIterator<Item = NetworkNodeConnectionConfig<ConnConf>>,
    ) -> Result<Self, ConnectionConfigGatherError> {
        use ConnectionConfigGatherError::*;

        Ok(NetworkNodeConnectionConfig {
            configs: remote_configs
                .into_iter()
                .enumerate()
                .map(|(i, config)| {
                    config
                        .configs
                        .get(i)
                        .cloned()
                        .ok_or(NotEnoughConnectionsFromNode { rank_id: i })
                })
                .collect::<Result<_, _>>()?,
        })
    }
}
