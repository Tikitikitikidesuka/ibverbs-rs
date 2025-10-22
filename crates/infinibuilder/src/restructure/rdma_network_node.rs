use super::rdma_connection::RdmaConnection;
use crate::restructure::barrier::RdmaNetworkBarrier;
use std::time::Duration;

pub trait RdmaNetworkNode<
    MR,
    RemoteMR,
    NB: RdmaNetworkBarrier<MR, RemoteMR>, /*, NT: RdmaNetworkTransport*/
>
{
    type Conn: RdmaConnection<MR = MR, RemoteMR = RemoteMR>;

    fn barrier<Group>(&mut self, group: &Group, timeout: Duration) -> Result<(), NB::Error>
    where
        Group: RdmaNetworkSelfGroup;

    // TODO: Transport methods
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

pub trait RdmaNetworkTransport {}
