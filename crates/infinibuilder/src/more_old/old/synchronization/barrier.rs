use crate::{IbBReadyNetworkConfig, IbBReadyNodeConfig};
use thiserror::Error;
use crate::synchronization::backend::IbBSynchronizationMasterBackend;
use crate::synchronization::barrier::IbBSyncBarrierError::{EmptyNetworkError, NodeNotInNetwork};

#[derive(Debug, Error)]
pub enum IbBSyncBarrierError {
    #[error("No nodes on the network config")]
    EmptyNetworkError,
    #[error("The node {0} is not part of the network")]
    NodeNotInNetwork(u32),
}

pub struct IbBSyncBarrier<'a> {
    self_rank_id: u32,
    network_config: &'a IbBReadyNetworkConfig,
    role: Role<'a>,
}

enum Role<'a> {
    Master(&'a mut IbBSynchronizationMasterBackend),
    Slave,
}

impl<'a> IbBSyncBarrier<'a> {
    pub fn new(
        rank_id: u32,
        network_config: &'a IbBReadyNetworkConfig,
        role: Role<'a>,
    ) -> Result<Self, IbBSyncBarrierError> {
        // Check the node is in the network
        if !network_config.contains_key(&rank_id) {
            return Err(NodeNotInNetwork(rank_id));
        }

        // Get master node (lowest node rank ids are guaranteed to be sorted)
        let master_node_config = network_config
            .iter()
            .next()
            .ok_or(EmptyNetworkError)?;

        // Check node's role
        let role = match rank_id == master_node_config.rank_id() {
            true => Role::Master,
            false => Role::Slave(master_node_config),
        };

        Ok(Self {
            self_rank_id: rank_id,
            network_config,
            role,
        })
    }

    // Poll without delay until sync is finished for minimum latency
    pub fn spin_poll_await(self) {

    }
}
