use crate::network::host::IbvNetworkRank;
use thiserror::Error;

pub mod host;
pub mod network_config;
pub mod prepared_network;
pub mod tcp_exchanger;

#[derive(Error, Debug)]
pub enum IbvNetworkHostError {
    #[error("Expected rank {expected} got {rank}")]
    RankMismatch {
        rank: IbvNetworkRank,
        expected: IbvNetworkRank,
    },
    #[error("Rank {rank} is not part of the network (0..num_peers)")]
    RankNotInNetwork {
        rank: IbvNetworkRank,
        num_peers: IbvNetworkRank,
    },
    #[error("Communication with self is not allowed.")]
    SelfConnection,

    #[error("Infiniband error occurred: {0}")]
    IoError(#[from] std::io::Error),
    /*
    #[error("Some errors occurred during a network multi-node operation: {0:?}")]
    MultiOperationError(Vec<IbvNetworkNodeError>),
    #[error("Barrier counter mismatch.")]
    BarrierMismatch,
    */
}
