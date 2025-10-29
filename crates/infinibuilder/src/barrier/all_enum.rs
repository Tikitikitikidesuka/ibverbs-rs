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
pub enum AnyUnregisteredBarrier<MR, RMR> {
    Centralized(UnregisteredCentralizedBarrier<MR, RMR>),
    BinaryTree(UnregisteredBinaryTreeBarrier<MR, RMR>),
    Dissemination(UnregisteredDisseminationBarrier<MR, RMR>),
}

#[derive(Debug)]
pub enum AnyBarrier<MR, RMR> {
    Centralized(CentralizedBarrier<MR, RMR>),
    BinaryTree(BinaryTreeBarrier<MR, RMR>),
    Dissemination(DisseminationBarrier<MR, RMR>),
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub enum AnyBarrierType {
    Centralized,
    BinaryTree,
    Dissemination,
}

impl<MR, RMR> RdmaNetworkMemoryRegionComponent<MR, RMR> for AnyUnregisteredBarrier<MR, RMR> {
    type Registered = AnyBarrier<MR, RMR>;
    type RegisterError = NonMatchingMemoryRegionCount;

    fn memory(&mut self, num_connections: usize) -> Vec<(*mut u8, usize)> {
        match self {
            AnyUnregisteredBarrier::Centralized(barrier) => barrier.memory(num_connections),
            AnyUnregisteredBarrier::BinaryTree(barrier) => barrier.memory(num_connections),
            AnyUnregisteredBarrier::Dissemination(barrier) => barrier.memory(num_connections),
        }
    }

    fn registered_mrs(self, mrs: Vec<MrPair<MR, RMR>>) -> Result<Self::Registered, Self::RegisterError> {
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

impl<MR, RMR> AnyBarrier<MR, RMR> {
    pub fn new(barrier_type: AnyBarrierType) -> AnyUnregisteredBarrier<MR, RMR> {
        match barrier_type {
            AnyBarrierType::Centralized => {
                AnyUnregisteredBarrier::Centralized(CentralizedBarrier::<MR, RMR>::new())
            }
            AnyBarrierType::BinaryTree => {
                AnyUnregisteredBarrier::BinaryTree(BinaryTreeBarrier::<MR, RMR>::new())
            }
            AnyBarrierType::Dissemination => {
                AnyUnregisteredBarrier::Dissemination(DisseminationBarrier::<MR, RMR>::new())
            }
        }
    }
}

impl<MR, RMR> RdmaNetworkBarrier<MR, RMR> for AnyBarrier<MR, RMR> {
    type Error = RdmaNetworkBarrierError;

    fn barrier<
        'network,
        Conn: RdmaConnection<MR, RMR> + 'network,
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
