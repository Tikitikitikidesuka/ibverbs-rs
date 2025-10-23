pub mod centralized;
pub mod binary_tree;
pub mod dissemination;

use std::error::Error;
use crate::restructure::rdma_connection::RdmaConnection;
use crate::restructure::rdma_network_node::RdmaNetworkSelfGroupConnections;
use std::time::Duration;
use thiserror::Error;

pub trait RdmaNetworkBarrier<MR, RemoteMR> {
    type Error;

    fn barrier<
        'network,
        Conn: RdmaConnection<MR = MR, RemoteMR = RemoteMR> + 'network,
        GroupConns: RdmaNetworkSelfGroupConnections<'network, Conn>,
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

    fn memory(&mut self, num_connections: usize) -> Vec<(*mut u8, usize)>;
    fn registered_mrs(self, mrs: Vec<(MR, RMR)>) -> Result<Self::Registered, Self::RegisterError>;
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
