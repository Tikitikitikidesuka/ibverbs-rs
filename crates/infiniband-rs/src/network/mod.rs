use crate::network::node::Rank;
use thiserror::Error;

mod memory_region;
pub mod network_config;
pub mod node;
pub mod prepared_node;
pub mod tcp_exchanger;
pub mod scatter_gather_element;

#[derive(Error, Debug)]
pub enum NodeError {
    #[error("Expected rank {expected} got {rank}")]
    RankMismatch { rank: Rank, expected: Rank },
    #[error("Rank {rank} is not part of the network (0..num_peers)")]
    RankNotInNetwork { rank: Rank, num_peers: Rank },
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
