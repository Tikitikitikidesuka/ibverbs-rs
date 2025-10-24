use crate::barrier::binary_tree::{BinaryTreeBarrier, UnregisteredBinaryTreeBarrier};
use crate::barrier::centralized::{CentralizedBarrier, UnregisteredCentralizedBarrier};
use crate::barrier::dissemination::{DisseminationBarrier, UnregisteredDisseminationBarrier};
use crate::barrier::{
    MrPair, NonMatchingMemoryRegionCount, RdmaNetworkBarrier, RdmaNetworkBarrierError,
    RdmaNetworkMemoryRegionComponent,
};
use crate::rdma_connection::RdmaConnection;
use crate::rdma_network_node::RdmaNetworkSelfGroupConnections;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug)]
pub enum AnyUnregisteredBarrier {
    Centralized(UnregisteredCentralizedBarrier),
    BinaryTree(UnregisteredBinaryTreeBarrier),
    Dissemination(UnregisteredDisseminationBarrier),
}

#[derive(Debug)]
pub enum AnyBarrier {
    Centralized(CentralizedBarrier),
    BinaryTree(BinaryTreeBarrier),
    Dissemination(DisseminationBarrier),
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub enum AnyBarrierType {
    Centralized,
    BinaryTree,
    Dissemination,
}

impl RdmaNetworkMemoryRegionComponent for AnyUnregisteredBarrier {
    type Registered = AnyBarrier;
    type RegisterError = NonMatchingMemoryRegionCount;

    fn memory(&mut self, num_connections: usize) -> Vec<(*mut u8, usize)> {
        match self {
            AnyUnregisteredBarrier::Centralized(barrier) => barrier.memory(num_connections),
            AnyUnregisteredBarrier::BinaryTree(barrier) => barrier.memory(num_connections),
            AnyUnregisteredBarrier::Dissemination(barrier) => barrier.memory(num_connections),
        }
    }

    fn registered_mrs(self, mrs: Vec<MrPair>) -> Result<Self::Registered, Self::RegisterError> {
        match self {
            AnyUnregisteredBarrier::Centralized(barrier) => {
                Ok(AnyBarrier::Centralized(barrier.registered_mrs(mrs)?))
            }
            AnyUnregisteredBarrier::BinaryTree(barrier) => {
                Ok(AnyBarrier::BinaryTree(barrier.registered_mrs(mrs)?))
            }
            AnyUnregisteredBarrier::Dissemination(barrier) => {
                Ok(AnyBarrier::Dissemination(barrier.registered_mrs(mrs)?))
            }
        }
    }
}

impl AnyBarrier {
    pub fn new(barrier_type: AnyBarrierType) -> AnyUnregisteredBarrier {
        match barrier_type {
            AnyBarrierType::Centralized => {
                AnyUnregisteredBarrier::Centralized(CentralizedBarrier::new())
            }
            AnyBarrierType::BinaryTree => {
                AnyUnregisteredBarrier::BinaryTree(BinaryTreeBarrier::new())
            }
            AnyBarrierType::Dissemination => {
                AnyUnregisteredBarrier::Dissemination(DisseminationBarrier::new())
            }
        }
    }
}

impl RdmaNetworkBarrier for AnyBarrier {
    type Error = RdmaNetworkBarrierError;

    fn barrier<
        'network,
        Conn: RdmaConnection + 'network,
        GroupConns: RdmaNetworkSelfGroupConnections<'network, Conn>,
    >(
        &mut self,
        connections: GroupConns,
        timeout: Duration,
    ) -> Result<(), Self::Error> {
        match self {
            AnyBarrier::Centralized(barrier) => barrier.barrier(connections, timeout),
            AnyBarrier::BinaryTree(barrier) => barrier.barrier(connections, timeout),
            AnyBarrier::Dissemination(barrier) => barrier.barrier(connections, timeout),
        }
    }
}
