use crate::barrier::RdmaNetworkBarrier;
use crate::rdma_connection::{RdmaConnection, RdmaWorkCompletion, RdmaWorkRequest};
use std::time::Duration;

// TODO: SEPARATE RdmaNetworkNode TRAIT INTO:
// - RdmaNetworkNode: rank_id()
// - RdmaBarrierNetworkNode: barrier()
// - RdmaSendReceiveNetworkNode: post_send(), post_receive()
// - RdmaReadWriteNetworkNode: post_write(), post_read()
// - RdmaImmDataNetworkNode: post_send_immediate_data(), post_receive_immediate_data()

pub trait RdmaNetworkNode<NB: RdmaNetworkBarrier/*, NT: RdmaNetworkTransport*/> {
    type Conn: RdmaConnection;
    /*
    type WR: RdmaWorkRequest;
    type WC: RdmaWorkCompletion;
     */

    fn rank_id(&self) -> usize;

    fn barrier<Group>(&mut self, group: &Group, timeout: Duration) -> Result<(), NB::Error>
    where
        Group: RdmaNetworkSelfGroup;

    /*
    fn post_send(
        &mut self,
        peer_rank_id: usize,
        memory_region: usize,
        memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<Self::WR, RdmaPostError>;

    // Posts a receive operation.
    fn post_receive(
        &mut self,
        peer_rank_id: usize,
        memory_region: usize,
        memory_range: impl RangeBounds<usize>,
    ) -> Result<Self::WR, RdmaPostError>;

    // Posts a write operation.
    // If sent with immediate data, the data must be obtained in the remote peer
    // by calling post_receive_immediate
    fn post_write(
        &mut self,
        peer_rank_id: usize,
        local_memory_region: usize,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: usize,
        remote_memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<Self::WR, RdmaPostError>;

    // Posts a read operation.
    fn post_read(
        &mut self,
        peer_rank_id: usize,
        local_memory_region: usize,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: usize,
        remote_memory_range: impl RangeBounds<usize>,
    ) -> Result<Self::WR, RdmaPostError>;

    fn post_send_immediate_data(
        &mut self,
        peer_rank_id: usize,
        immediate_data: u32,
    ) -> Result<IbvWorkRequest, std::io::Error>;

    fn post_receive_immediate_data(
        &mut self,
        peer_rank_id: usize,
    ) -> Result<IbvWorkRequest, std::io::Error>;
    */
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
