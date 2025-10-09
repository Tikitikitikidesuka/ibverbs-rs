use crate::connect::Connect;
use crate::rdma_traits::{RdmaSync, RdmaSendRecv};
use derivative::Derivative;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::ops::Range;
use std::sync::atomic::AtomicUsize;
use thiserror::Error;

pub struct UnconnectedNetworkNode<Conn: Connect> {
    pub(crate) rank_id: usize,
    pub(crate) connections: Vec<Conn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkNodeConnectionConfig<ConnConf> {
    configs: Vec<ConnConf>,
}

pub struct ConnectedNetworkNode<T> {
    rank_id: usize,
    connections: Vec<T>,
}

impl<Conn: Connect<Connected = T, ConnectionConfig = ConnConf>, T, ConnConf> Connect
    for UnconnectedNetworkNode<Conn>
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
            rank_id: self.rank_id,
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
    pub fn gather<'a>(
        rank_id: usize,
        remote_configs: impl IntoIterator<Item = &'a NetworkNodeConnectionConfig<ConnConf>>,
    ) -> Result<Self, ConnectionConfigGatherError>
    where
        ConnConf: 'a,
    {
        use ConnectionConfigGatherError::*;
        Ok(NetworkNodeConnectionConfig {
            configs: remote_configs
                .into_iter()
                .enumerate()
                .map(|(i, config)| {
                    config
                        .configs
                        .get(rank_id)
                        .cloned()
                        .ok_or(NotEnoughConnectionsFromNode { rank_id: i })
                })
                .collect::<Result<_, _>>()?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct NetworkGroup {
    // Guarantees to be sorted for efficiency of operations
    rank_ids: Vec<usize>,
    network_rank_ids: Range<usize>,
    network_id: usize,
}

#[derive(Debug, Error, Copy, Clone)]
#[error("Node {rank_id} not in network")]
pub struct NodeNotInNetwork {
    rank_id: usize,
}

#[derive(Debug, Error, Copy, Clone)]
#[error("Group does not belong to the network")]
pub struct NonMatchingNetwork;

impl NetworkGroup {
    pub fn members(&self) -> &[usize] {
        self.rank_ids.as_slice()
    }

    pub fn union<G: Borrow<Self>>(&self, other: G) -> Result<Self, NonMatchingNetwork> {
        let other = other.borrow();

        if self.network_id != other.network_id {
            return Err(NonMatchingNetwork);
        }

        let ids = self
            .rank_ids
            .iter()
            .merge(other.rank_ids.iter())
            .cloned()
            .dedup()
            .collect();

        Ok(NetworkGroup {
            rank_ids: ids,
            network_rank_ids: self.network_rank_ids.clone(),
            network_id: self.network_id,
        })
    }

    pub fn intersection<G: Borrow<Self>>(&self, other: G) -> Result<Self, NonMatchingNetwork> {
        let other = other.borrow();

        if self.network_id != other.network_id {
            return Err(NonMatchingNetwork);
        }

        let ids = self
            .rank_ids
            .iter()
            .merge_join_by(other.rank_ids.iter(), |a, b| a.cmp(b))
            .filter_map(|either| match either {
                itertools::EitherOrBoth::Both(&a, _) => Some(a),
                _ => None,
            })
            .collect();

        Ok(NetworkGroup {
            rank_ids: ids,
            network_rank_ids: self.network_rank_ids.clone(),
            network_id: self.network_id,
        })
    }

    pub fn difference<G: Borrow<Self>>(&self, other: G) -> Result<Self, NonMatchingNetwork> {
        let other = other.borrow();

        if self.network_id != other.network_id {
            return Err(NonMatchingNetwork);
        }

        let ids = self
            .rank_ids
            .iter()
            .merge_join_by(other.rank_ids.iter(), |a, b| a.cmp(b))
            .filter_map(|either| match either {
                itertools::EitherOrBoth::Left(&a) => Some(a),
                _ => None,
            })
            .collect();

        Ok(NetworkGroup {
            rank_ids: ids,
            network_rank_ids: self.network_rank_ids.clone(),
            network_id: self.network_id,
        })
    }

    pub fn complement(&self) -> Self {
        let ids = self
            .network_rank_ids
            .clone()
            .into_iter()
            .merge_join_by(self.rank_ids.iter(), |a, b| a.cmp(b))
            .filter_map(|either| match either {
                itertools::EitherOrBoth::Left(a) => Some(a),
                _ => None,
            })
            .collect();

        NetworkGroup {
            rank_ids: ids,
            network_rank_ids: self.network_rank_ids.clone(),
            network_id: self.network_id,
        }
    }
}

pub trait NetworkOp {
    type Output;

    // Runs a network operation on a certain group
    // If the current node is in the group, group_idx represents
    // the index its own connection in  group_connections
    fn run<'a, T: 'a + RdmaSendRecv + RdmaSync>(
        &self,
        self_idx: Option<usize>,
        group_connections: &mut [&'a mut T],
    ) -> Self::Output;
}

impl<T> ConnectedNetworkNode<T> {
    pub fn rank_id(&self) -> usize {
        self.rank_id
    }
}

impl<T: RdmaSendRecv + RdmaSync> ConnectedNetworkNode<T> {
    pub fn connection(&mut self, rank_id: usize) -> Option<&mut T> {
        self.connections.get_mut(rank_id)
    }

    pub fn connections<'a>(
        &'a mut self,
        group: &'a NetworkGroup,
    ) -> Result<Vec<&'a mut T>, NonMatchingNetwork> {
        if self.rank_id != group.network_id {
            return Err(NonMatchingNetwork);
        }

        let ptr = self.connections.as_mut_ptr();
        Ok(group
            .rank_ids
            .iter()
            .map(move |&rank_id| unsafe { &mut *ptr.add(rank_id) })
            .collect())
    }

    pub fn group<I>(&self, rank_ids: I) -> Result<NetworkGroup, NodeNotInNetwork>
    where
        I: IntoIterator,
        I::Item: Borrow<usize>,
    {
        let mut ids: Vec<usize> = rank_ids.into_iter().map(|id| *id.borrow()).collect();

        // Sort & deduplicate
        ids.sort();
        ids.dedup();

        // Validate: return first invalid id
        for &id in &ids {
            if id >= self.connections.len() {
                return Err(NodeNotInNetwork { rank_id: id });
            }
        }

        Ok(NetworkGroup {
            rank_ids: ids,
            network_rank_ids: 0..self.connections.len(),
            network_id: self.rank_id,
        })
    }

    pub fn group_all(&self) -> NetworkGroup {
        self.group(0..self.connections.len()).unwrap()
    }

    pub fn group_others(&self) -> NetworkGroup {
        self.group_all()
            .difference(self.group(&[self.rank_id]).unwrap())
            .unwrap()
    }

    pub fn run<O: NetworkOp>(
        &mut self,
        network_op: &O,
        group: &NetworkGroup,
    ) -> Result<O::Output, NonMatchingNetwork> {
        let group_idx = group.rank_ids.binary_search(&self.rank_id).ok();
        Ok(network_op.run(group_idx, self.connections(group)?.as_mut_slice()))
    }
}
