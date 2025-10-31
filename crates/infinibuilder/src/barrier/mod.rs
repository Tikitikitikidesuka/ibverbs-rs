pub mod all_enum;
pub mod binary_tree;
pub mod centralized;
pub mod dissemination;

use crate::rdma_network_node::RdmaNetworkSelfGroupConnections;
use std::error::Error;
use std::fmt::Debug;
use std::time::Duration;
use thiserror::Error;
use crate::rdma_connection::RdmaConnection;

pub trait RdmaNetworkBarrier<ConnMR, ConnRMR> {
    type Error: Error;

    fn barrier<
        'network,
        Conn: RdmaConnection<ConnMR, ConnRMR> + 'network,
        GroupConns: RdmaNetworkSelfGroupConnections<'network, ConnMR, ConnRMR, Conn>,
    >(
        &mut self,
        connections: GroupConns,
        timeout: Duration,
    ) -> Result<(), Self::Error>;
}

/// This trait is defined to be able to let a network component have memory regions for the connections.
/// It must make possible telling the component how many connections there are.
/// It must the allow getting the memory for each of them.
/// Finally, it must allow giving the component the registered memory regions.
pub trait RdmaNetworkMemoryRegionComponent<MR, RMR> {
    type Registered;
    type RegisterError: Error;

    fn memory(&mut self, num_connections: usize) -> Option<Vec<(*mut u8, usize)>>;
    fn registered_mrs(
        self,
        mrs: Option<Vec<MemoryRegionPair<MR, RMR>>>,
    ) -> Result<Self::Registered, Self::RegisterError>;
}

#[derive(Debug, Copy, Clone)]
pub struct MemoryRegionPair<MR, RMR> {
    pub local_mr: MR,
    pub remote_mr: RMR,
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
