use crate::ibverbs::simple_unit::sync_transfer_mode::SyncTransferMode;
use crate::ibverbs::simple_unit::{IbvSimpleUnit, UnconnectedIbvSimpleUnit};
use crate::network::{ConnectedNetworkNode, UnconnectedNetworkNode};
use crate::network_config::NetworkConfig;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IbvSimpleUnitNetworkCreationError {
    #[error("Rank id {rank_id} not found in network config")]
    RankIdNotFound { rank_id: usize },
    #[error("Ibv device \"{ibvdev}\" not found")]
    IbvDeviceNotFound { ibvdev: String },
    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

impl ConnectedNetworkNode<IbvSimpleUnit<SyncTransferMode<0>>> {
    pub fn new_ibv_simple_unit_network_node<const CQ_SIZE: usize, const POLL_BUFF_SIZE: usize>(
        rank_id: usize,
        network_config: &NetworkConfig,
        memory: &[u8],
    ) -> Result<
        UnconnectedNetworkNode<UnconnectedIbvSimpleUnit<SyncTransferMode<POLL_BUFF_SIZE>>>,
        IbvSimpleUnitNetworkCreationError,
    > {
        use IbvSimpleUnitNetworkCreationError::*;

        let node_config = network_config
            .get(rank_id)
            .ok_or(RankIdNotFound { rank_id })?;

        let ibv_context = ibverbs::devices()?
            .iter()
            .find(|d| match d.name() {
                None => false,
                Some(name) => name.to_string_lossy() == node_config.ibdev,
            })
            .ok_or(IbvDeviceNotFound {
                ibvdev: node_config.ibdev.clone(),
            })?
            .open()?;

        Ok(UnconnectedNetworkNode {
            rank_id,
            connections: network_config
                .iter()
                .map(|_| unsafe {
                    IbvSimpleUnit::new_sync_transfer_unit::<CQ_SIZE, POLL_BUFF_SIZE>(
                        &ibv_context,
                        &memory,
                    )
                })
                .collect::<Result<_, _>>()?,
        })
    }
}
