use crate::barrier::binary_tree::{BinaryTreeBarrier, UnregisteredBinaryTreeBarrier};
use crate::barrier::centralized::{CentralizedBarrier, UnregisteredCentralizedBarrier};
use crate::barrier::dissemination::{DisseminationBarrier, UnregisteredDisseminationBarrier};
use crate::barrier::{
    RdmaNetworkNodeBarrier, RdmaNetworkNodeBarrierError,
};
use crate::rdma_connection::RdmaConnection;
use crate::rdma_network_node::{MemoryRegionPair, NonMatchingMemoryRegionCount, RdmaNetworkMemoryRegionComponent, RdmaNetworkSelfGroupConnections};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug)]
pub enum AnyUnregisteredBarrier<Connection: RdmaConnection> {
    Centralized(UnregisteredCentralizedBarrier<Connection>),
    BinaryTree(UnregisteredBinaryTreeBarrier<Connection>),
    Dissemination(UnregisteredDisseminationBarrier<Connection>),
}

#[derive(Debug)]
pub enum AnyBarrier<Connection: RdmaConnection> {
    Centralized(CentralizedBarrier<Connection>),
    BinaryTree(BinaryTreeBarrier<Connection>),
    Dissemination(DisseminationBarrier<Connection>),
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub enum AnyBarrierType {
    Centralized,
    BinaryTree,
    Dissemination,
}

impl<Connection: RdmaConnection>
    RdmaNetworkMemoryRegionComponent<Connection::MemoryRegion, Connection::RemoteMemoryRegion>
    for AnyUnregisteredBarrier<Connection>
{
    type Registered = AnyBarrier<Connection>;
    type RegisterError = NonMatchingMemoryRegionCount;

    fn memory(&mut self, num_connections: usize) -> Option<Vec<(*mut u8, usize)>> {
        match self {
            AnyUnregisteredBarrier::Centralized(barrier) => barrier.memory(num_connections),
            AnyUnregisteredBarrier::BinaryTree(barrier) => barrier.memory(num_connections),
            AnyUnregisteredBarrier::Dissemination(barrier) => barrier.memory(num_connections),
        }
    }

    fn registered_mrs(
        self,
        mrs: Option<
            Vec<MemoryRegionPair<Connection::MemoryRegion, Connection::RemoteMemoryRegion>>,
        >,
    ) -> Result<Self::Registered, Self::RegisterError> {
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

impl<Connection: RdmaConnection> AnyBarrier<Connection> {
    pub fn new(barrier_type: AnyBarrierType) -> AnyUnregisteredBarrier<Connection> {
        match barrier_type {
            AnyBarrierType::Centralized => {
                AnyUnregisteredBarrier::Centralized(CentralizedBarrier::<Connection>::new())
            }
            AnyBarrierType::BinaryTree => {
                AnyUnregisteredBarrier::BinaryTree(BinaryTreeBarrier::<Connection>::new())
            }
            AnyBarrierType::Dissemination => {
                AnyUnregisteredBarrier::Dissemination(DisseminationBarrier::<Connection>::new())
            }
        }
    }
}

impl<Connection: RdmaConnection> RdmaNetworkNodeBarrier<Connection> for AnyBarrier<Connection> {
    type Error = RdmaNetworkNodeBarrierError;

    fn barrier<
        'network,
        GroupConns: RdmaNetworkSelfGroupConnections<'network, Connection = Connection>,
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
