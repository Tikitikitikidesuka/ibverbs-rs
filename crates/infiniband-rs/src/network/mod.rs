use crate::network::host::IbvNetworkRank;
use thiserror::Error;

pub mod network_config;
pub mod host;
mod prepared_network;
pub mod tcp_exchanger;

#[derive(Error, Debug)]
pub enum IbvNetworkNodeError {
    #[error("Communication with self is not allowed.")]
    SelfConnection,
    #[error("Rank {rank} is not part of the network. It must be in {:?}\
     and not equal to the own rank", 0..*num_peers)]
    RankNotInNetwork {
        rank: IbvNetworkRank,
        num_peers: IbvNetworkRank,
    },
    #[error("Infiniband error occurred: {0}")]
    IoError(#[from] std::io::Error),
    /*
    #[error("Some errors occurred during a network multi-node operation: {0:?}")]
    MultiOperationError(Vec<IbvNetworkNodeError>),
    #[error("Barrier counter mismatch.")]
    BarrierMismatch,
    */
}
