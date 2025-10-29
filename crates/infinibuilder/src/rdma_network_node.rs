use crate::barrier::RdmaNetworkBarrier;
use crate::rdma_connection::{
    RdmaConnection, RdmaMemoryRegion, RdmaRemoteMemoryRegion, RdmaWorkRequest,
};
use std::error::Error;
use std::ops::RangeBounds;
use std::time::Duration;

pub trait RdmaNetworkNode {
    type Group: RdmaNetworkGroup;
    type SelfGroup: RdmaNetworkSelfGroup;

    fn rank_id(&self) -> usize;

    /// Group of all including self
    fn group_all(&self) -> Self::SelfGroup;

    /// Group of all except self
    fn group_peers(&self) -> Self::Group;
}

pub trait RdmaBarrierNetworkNode<NB: RdmaNetworkBarrier> {
    fn barrier<Group>(&mut self, group: &Group, timeout: Duration) -> Result<(), NB::Error>
    where
        Group: RdmaNetworkSelfGroup;
}

pub trait RdmaTransportSendReceiveNetworkNode {
    type WR: RdmaWorkRequest;
    type PostError: Error;

    fn post_send(
        &mut self,
        peer_rank_id: usize,
        memory_region: RdmaMemoryRegion,
        memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<Self::WR, Self::PostError>;

    // Posts a receive operation.
    fn post_receive(
        &mut self,
        peer_rank_id: usize,
        memory_region: RdmaMemoryRegion,
        memory_range: impl RangeBounds<usize>,
    ) -> Result<Self::WR, Self::PostError>;
}

pub trait RdmaTransportReadWriteNetworkNode {
    type WR: RdmaWorkRequest;
    type PostError: Error;

    // Posts a write operation.
    // If sent with immediate data, the data must be obtained in the remote peer
    // by calling post_receive_immediate
    fn post_write(
        &mut self,
        peer_rank_id: usize,
        local_memory_region: RdmaMemoryRegion,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: RdmaRemoteMemoryRegion,
        remote_memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<Self::WR, Self::PostError>;

    // Posts a read operation.
    fn post_read(
        &mut self,
        peer_rank_id: usize,
        local_memory_region: RdmaMemoryRegion,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: RdmaRemoteMemoryRegion,
        remote_memory_range: impl RangeBounds<usize>,
    ) -> Result<Self::WR, Self::PostError>;
}

pub trait RdmaTransportImmediateDataNetworkNode {
    type WR: RdmaWorkRequest;
    type PostError: Error;

    fn post_send_immediate_data(
        &mut self,
        peer_rank_id: usize,
        immediate_data: u32,
    ) -> Result<Self::WR, Self::PostError>;

    fn post_receive_immediate_data(
        &mut self,
        peer_rank_id: usize,
    ) -> Result<Self::WR, Self::PostError>;
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

pub trait RdmaNetworkGroupConnections<'network, Conn: RdmaConnection + 'network>:
    RdmaNetworkGroup
{
    fn connection_mut(&mut self, idx: usize) -> Option<&'network mut Conn>;
}

pub trait RdmaNetworkSelfGroupConnections<'network, Conn: RdmaConnection + 'network>:
    RdmaNetworkSelfGroup
{
    fn connection_mut(&mut self, idx: usize) -> Option<RdmaNetworkSelfGroupConnection<Conn>>;
}

pub enum RdmaNetworkSelfGroupConnection<'network, Conn: RdmaConnection> {
    SelfConnection,
    PeerConnection(usize, &'network mut Conn),
}
