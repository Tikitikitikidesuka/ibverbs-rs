pub mod all_enum;
pub mod binary_tree;
pub mod centralized;
pub mod dissemination;

use crate::rdma_connection::RdmaConnection;
use crate::rdma_network_node::RdmaNetworkSelfGroupConnections;
use std::error::Error;
use std::fmt::Debug;
use std::time::Duration;
use thiserror::Error;

pub trait RdmaNetworkNodeBarrier<Connection: RdmaConnection> {
    type Error: Error;

    fn barrier<
        'network,
        GroupConns: RdmaNetworkSelfGroupConnections<'network, Connection = Connection>,
    >(
        &mut self,
        connections: GroupConns,
        timeout: Duration,
    ) -> Result<(), Self::Error>;
}

#[derive(Debug, Error)]
pub enum RdmaNetworkNodeBarrierError {
    #[error("Centralized barrier timeout: {0}:")]
    Timeout(String),
    #[error("Centralized barrier RDMA error: {0}")]
    RdmaError(String),
}

