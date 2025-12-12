use thiserror::Error;

pub mod node;

#[derive(Error, Debug)]
pub enum NetworkNodeError {
    #[error("Communication with self is not allowed.")]
    SelfConnection,
    #[error("Peer {specified} does not exist, peer must be in {:?} and not equal to the own rank", 0..*num_peers)]
    PeerOutOfBounds { specified: usize, num_peers: usize },

    #[error("Infiniband error occurred: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Some errors occured during a network multi-node operation: {0:?}")]
    MultiOperationError(Vec<NetworkNodeError>),
}

pub type Result<T = ()> = std::result::Result<T, NetworkNodeError>;
