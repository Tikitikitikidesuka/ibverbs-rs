//pub mod all_enum;
//pub mod binary_tree;
pub mod centralized;
//pub mod dissemination;

use crate::rdma_network_node::RdmaNetworkSelfGroupConnections;
use std::error::Error;
use std::fmt::Debug;
use std::time::Duration;
use thiserror::Error;
use crate::rdma_connection::RdmaConnection;

pub trait RdmaNetworkBarrier {
    type Error: Error;

    fn barrier<
        'network,
        GroupConns: RdmaNetworkSelfGroupConnections<'network>,
    >(
        &mut self,
        connections: GroupConns,
        timeout: Duration,
    ) -> Result<(), Self::Error>;
}

#[derive(Debug, Error)]
pub enum RdmaNetworkBarrierError {
    #[error("Centralized barrier timeout: {0}:")]
    Timeout(String),
    #[error("Centralized barrier RDMA error: {0}")]
    RdmaError(String),
}

#[derive(Debug, Error)]
#[error("Non matching memory region count, expected {expected}, got {got}")]
pub struct NonMatchingMemoryRegionCount {
    expected: usize,
    got: usize,
}
