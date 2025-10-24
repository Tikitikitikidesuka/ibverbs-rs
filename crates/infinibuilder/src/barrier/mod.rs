pub mod all_enum;
pub mod binary_tree;
pub mod centralized;
pub mod dissemination;

use crate::rdma_connection::RdmaConnection;
use crate::rdma_network_node::RdmaNetworkSelfGroupConnections;
use std::error::Error;
use std::time::Duration;
use thiserror::Error;

pub trait RdmaNetworkBarrier {
    type Error: Error;

    fn barrier<
        'network,
        Conn: RdmaConnection + 'network,
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
pub trait RdmaNetworkMemoryRegionComponent {
    type Registered;
    type RegisterError: Error;

    fn memory(&mut self, num_connections: usize) -> Vec<(*mut u8, usize)>;
    fn registered_mrs(
        self,
        mrs: Vec<MrPair>,
    ) -> Result<Self::Registered, Self::RegisterError>;
}

#[derive(Debug, Copy, Clone)]
pub struct MrPair {
    pub local_mr_idx: usize,
    pub remote_mr_idx: usize,
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
