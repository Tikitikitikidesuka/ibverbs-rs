use crate::barrier::RdmaNetworkNodeBarrier;
use crate::rdma_connection::{
    RdmaConnection, RdmaPostReceiveImmediateDataConnection, RdmaPostSendImmediateDataConnection,
    RdmaWorkRequest,
};
use std::error::Error;
use std::ops::RangeBounds;
use std::time::Duration;

pub trait RdmaNetworkNode:
    RdmaRankIdNetworkNode
    + RdmaMemoryRegionNetworkNode
    + RdmaRemoteMemoryRegionNetworkNode
    + RdmaNamedMemoryRegionNetworkNode
    + RdmaGroupNetworkNode
    + RdmaBarrierNetworkNode
    + RdmaSendTransportNetworkNode
    + RdmaReceiveTransportNetworkNode
    + RdmaReadTransportNetworkNode
    + RdmaWriteTransportNetworkNode
    + RdmaSendImmediateDataTransportNetworkNode
    + RdmaReceiveImmediateDataTransportNetworkNode
{
}

// Blanket implementation
impl<NetworkNode> RdmaNetworkNode for NetworkNode where
    NetworkNode: RdmaRankIdNetworkNode
        + RdmaMemoryRegionNetworkNode
        + RdmaRemoteMemoryRegionNetworkNode
        + RdmaNamedMemoryRegionNetworkNode
        + RdmaGroupNetworkNode
        + RdmaBarrierNetworkNode
        + RdmaSendTransportNetworkNode
        + RdmaReceiveTransportNetworkNode
        + RdmaReadTransportNetworkNode
        + RdmaWriteTransportNetworkNode
        + RdmaSendImmediateDataTransportNetworkNode
        + RdmaReceiveImmediateDataTransportNetworkNode
{
}

pub trait RdmaRankIdNetworkNode {
    fn rank_id(&self) -> usize;
}

pub trait RdmaGroupNetworkNode {
    type Group: RdmaNetworkGroup;
    type SelfGroup: RdmaNetworkSelfGroup;

    /// Group of all including self
    fn group_all(&self) -> Self::SelfGroup;

    /// Group of all except self
    fn group_peers(&self) -> Self::Group;
}

pub trait RdmaBarrierNetworkNode {
    type BarrierError: Error;

    fn barrier<Group>(
        &mut self,
        group: &Group,
        timeout: Duration,
    ) -> Result<(), Self::BarrierError>
    where
        Group: RdmaNetworkSelfGroup;
}

pub trait RdmaMemoryRegionNetworkNode {
    type MemoryRegion;
}

pub trait RdmaNamedMemoryRegionNetworkNode: RdmaMemoryRegionNetworkNode {
    fn local_mr(&self, id: impl AsRef<str>) -> Option<Self::MemoryRegion>;
}

pub trait RdmaRemoteMemoryRegionNetworkNode {
    type RemoteMemoryRegion;
}

pub trait RdmaNamedRemoteMemoryRegionNetworkNode: RdmaRemoteMemoryRegionNetworkNode {
    fn remote_mr(&self, id: impl AsRef<str>) -> Option<Self::RemoteMemoryRegion>;
}

pub trait RdmaSendTransportNetworkNode:
    RdmaMemoryRegionNetworkNode
{
    type WorkRequest: RdmaWorkRequest;
    type PostError: Error;

    fn post_send(
        &mut self,
        peer_rank_id: usize,
        memory_region: &Self::MemoryRegion,
        memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<Self::WorkRequest, Self::PostError>;
}

pub trait RdmaReceiveTransportNetworkNode:
    RdmaMemoryRegionNetworkNode
{
    type WorkRequest: RdmaWorkRequest;
    type PostError: Error;

    fn post_receive(
        &mut self,
        peer_rank_id: usize,
        memory_region: &Self::MemoryRegion,
        memory_range: impl RangeBounds<usize>,
    ) -> Result<Self::WorkRequest, Self::PostError>;
}

pub trait RdmaWriteTransportNetworkNode:
    RdmaMemoryRegionNetworkNode + RdmaRemoteMemoryRegionNetworkNode
{
    type WorkRequest: RdmaWorkRequest;
    type PostError: Error;

    fn post_write(
        &mut self,
        peer_rank_id: usize,
        local_memory_region: &Self::MemoryRegion,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: &Self::RemoteMemoryRegion,
        remote_memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<Self::WorkRequest, Self::PostError>;
}

pub trait RdmaReadTransportNetworkNode:
    RdmaMemoryRegionNetworkNode + RdmaRemoteMemoryRegionNetworkNode
{
    type WorkRequest: RdmaWorkRequest;
    type PostError: Error;

    fn post_read(
        &mut self,
        peer_rank_id: usize,
        local_memory_region: &Self::MemoryRegion,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: &Self::RemoteMemoryRegion,
        remote_memory_range: impl RangeBounds<usize>,
    ) -> Result<Self::WorkRequest, Self::PostError>;
}

pub trait RdmaSendImmediateDataTransportNetworkNode {
    type WorkRequest: RdmaWorkRequest;
    type PostError: Error;

    fn post_send_immediate_data(
        &mut self,
        peer_rank_id: usize,
        immediate_data: u32,
    ) -> Result<Self::WorkRequest, Self::PostError>;
}

pub trait RdmaReceiveImmediateDataTransportNetworkNode {
    type WorkRequest: RdmaWorkRequest;
    type PostError: Error;

    fn post_receive_immediate_data(
        &mut self,
        peer_rank_id: usize,
    ) -> Result<Self::WorkRequest, Self::PostError>;
}

#[derive(Debug, Clone)]
pub struct RdmaNamedMemory {
    pub(super) id: String,
    pub(super) ptr: *mut u8,
    pub(super) length: usize,
}

impl RdmaNamedMemory {
    pub fn new(id: impl Into<String>, ptr: *mut u8, length: usize) -> Self {
        Self {
            id: id.into(),
            ptr,
            length,
        }
    }
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

// A group of nodes of the network by rank id
// Order of the rank ids matter since the nodes in a group are addressed by index
// Two groups with the same rank ids but different order are not equivalent
// Operations count on the groups being equivalent in different nodes when operating together
pub trait RdmaNetworkGroup {
    fn len(&self) -> usize;

    fn rank_ids(&self) -> &[usize];

    fn rank_id(&self, idx: usize) -> Option<usize>;
}

// Same as `RdmaNetworkGroup` but the local node is guaranteed to be a part of it
// It is used for communications that require a group operation like barrier
pub trait RdmaNetworkSelfGroup: RdmaNetworkGroup {
    fn self_idx(&self) -> usize;

    fn self_rank_id(&self) -> usize {
        self.rank_id(self.self_idx()).unwrap()
    }
}

pub trait RdmaNetworkGroupConnections<'network>: RdmaNetworkGroup {
    type Connection: RdmaConnection;

    fn connection_mut(&mut self, idx: usize) -> Option<&'network mut Self::Connection>;
}

pub trait RdmaNetworkSelfGroupConnections<'network>: RdmaNetworkSelfGroup {
    type Connection: RdmaConnection;

    fn connection_mut(&mut self, idx: usize) -> Option<RdmaNetworkSelfGroupConnection<Self::Connection>>;
}

pub enum RdmaNetworkSelfGroupConnection<'network, Conn> {
    SelfConnection,
    PeerConnection(usize, &'network mut Conn),
}
