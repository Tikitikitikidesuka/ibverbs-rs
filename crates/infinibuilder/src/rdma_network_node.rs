use crate::rdma_connection::{RdmaConnection, RdmaWorkRequest};
use std::borrow::Borrow;
use std::error::Error;
use std::ops::RangeBounds;
use std::time::Duration;
use thiserror::Error;

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

pub struct RdmaSendParams<'a, MemoryRegion, Range> {
    pub memory_region: &'a MemoryRegion,
    pub memory_range: Range,
    pub immediate_data: Option<u32>,
}

pub trait RdmaSendTransportNetworkNode: RdmaMemoryRegionNetworkNode {
    type WorkRequest: RdmaWorkRequest;
    type PostError: Error;

    fn post_send(
        &mut self,
        peer_rank_id: usize,
        memory_region: &Self::MemoryRegion,
        memory_range: impl RangeBounds<usize> + Clone,
        immediate_data: Option<u32>,
    ) -> Result<Self::WorkRequest, Self::PostError>;

    fn post_send_batch<'a, Range: RangeBounds<usize> + Clone>(
        &mut self,
        peer_rank_id: usize,
        send_params_iter: impl IntoIterator<
            Item = impl Borrow<RdmaSendParams<'a, Self::MemoryRegion, Range>>,
        >,
    ) -> impl Iterator<Item = Result<Self::WorkRequest, Self::PostError>>
    where
        <Self as RdmaMemoryRegionNetworkNode>::MemoryRegion: 'a,
    {
        send_params_iter.into_iter().map(move |send_params| {
            let send_params = send_params.borrow();
            self.post_send(
                peer_rank_id,
                send_params.memory_region,
                send_params.memory_range.clone(),
                send_params.immediate_data,
            )
        })
    }
}

pub struct RdmaReceiveParams<'a, MemoryRegion, Range> {
    pub memory_region: &'a MemoryRegion,
    pub memory_range: Range,
}

pub trait RdmaReceiveTransportNetworkNode: RdmaMemoryRegionNetworkNode {
    type WorkRequest: RdmaWorkRequest;
    type PostError: Error;

    fn post_receive(
        &mut self,
        peer_rank_id: usize,
        memory_region: &Self::MemoryRegion,
        memory_range: impl RangeBounds<usize> + Clone,
    ) -> Result<Self::WorkRequest, Self::PostError>;

    fn post_receive_batch<'a, Range: RangeBounds<usize> + Clone>(
        &mut self,
        peer_rank_id: usize,
        receive_params_iter: impl IntoIterator<
            Item = impl Borrow<RdmaReceiveParams<'a, Self::MemoryRegion, Range>>,
        >,
    ) -> impl Iterator<Item = Result<Self::WorkRequest, Self::PostError>>
    where
        <Self as RdmaMemoryRegionNetworkNode>::MemoryRegion: 'a,
    {
        receive_params_iter.into_iter().map(move |receive_params| {
            let receive_params = receive_params.borrow();
            self.post_receive(
                peer_rank_id,
                receive_params.memory_region,
                receive_params.memory_range.clone(),
            )
        })
    }
}

pub struct RdmaWriteParams<'a, MemoryRegion, RemoteMemoryRegion, Range> {
    pub local_memory_region: &'a MemoryRegion,
    pub local_memory_range: Range,
    pub remote_memory_region: &'a RemoteMemoryRegion,
    pub remote_memory_range: Range,
    pub immediate_data: Option<u32>,
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
        local_memory_range: impl RangeBounds<usize> + Clone,
        remote_memory_region: &Self::RemoteMemoryRegion,
        remote_memory_range: impl RangeBounds<usize> + Clone,
        immediate_data: Option<u32>,
    ) -> Result<Self::WorkRequest, Self::PostError>;

    fn post_write_batch<'a, Range: RangeBounds<usize> + Clone>(
        &mut self,
        peer_rank_id: usize,
        write_params_iter: impl IntoIterator<
            Item = impl Borrow<RdmaWriteParams<'a, Self::MemoryRegion, Self::RemoteMemoryRegion, Range>>,
        >,
    ) -> impl Iterator<Item = Result<Self::WorkRequest, Self::PostError>>
    where
        <Self as RdmaMemoryRegionNetworkNode>::MemoryRegion: 'a,
        <Self as RdmaRemoteMemoryRegionNetworkNode>::RemoteMemoryRegion: 'a,
    {
        write_params_iter.into_iter().map(move |write_params| {
            let write_params = write_params.borrow();
            self.post_write(
                peer_rank_id,
                write_params.local_memory_region,
                write_params.local_memory_range.clone(),
                write_params.remote_memory_region,
                write_params.remote_memory_range.clone(),
                write_params.immediate_data,
            )
        })
    }
}

pub struct RdmaReadParams<'a, MemoryRegion, RemoteMemoryRegion, Range> {
    pub local_memory_region: &'a MemoryRegion,
    pub local_memory_range: Range,
    pub remote_memory_region: &'a RemoteMemoryRegion,
    pub remote_memory_range: Range,
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
        local_memory_range: impl RangeBounds<usize> + Clone,
        remote_memory_region: &Self::RemoteMemoryRegion,
        remote_memory_range: impl RangeBounds<usize> + Clone,
    ) -> Result<Self::WorkRequest, Self::PostError>;

    fn post_read_batch<'a, Range: RangeBounds<usize> + Clone>(
        &mut self,
        peer_rank_id: usize,
        read_params_iter: impl IntoIterator<
            Item = impl Borrow<RdmaReadParams<'a, Self::MemoryRegion, Self::RemoteMemoryRegion, Range>>,
        >,
    ) -> Vec<Result<Self::WorkRequest, Self::PostError>>
    where
        <Self as RdmaMemoryRegionNetworkNode>::MemoryRegion: 'a,
        <Self as RdmaRemoteMemoryRegionNetworkNode>::RemoteMemoryRegion: 'a,
    {
        read_params_iter
            .into_iter()
            .map(|read_params| {
                let read_params = read_params.borrow();
                self.post_read(
                    peer_rank_id,
                    read_params.local_memory_region,
                    read_params.local_memory_range.clone(),
                    read_params.remote_memory_region,
                    read_params.remote_memory_range.clone(),
                )
            })
            .collect()
    }
}

pub trait RdmaSendImmediateDataTransportNetworkNode {
    type WorkRequest: RdmaWorkRequest;
    type PostError: Error;

    fn post_send_immediate_data(
        &mut self,
        peer_rank_id: usize,
        immediate_data: u32,
    ) -> Result<Self::WorkRequest, Self::PostError>;

    fn post_send_immediate_data_batch<Range: RangeBounds<usize> + Clone>(
        &mut self,
        peer_rank_id: usize,
        send_immediate_data_params_iter: &[u32],
    ) -> Vec<Result<Self::WorkRequest, Self::PostError>> {
        send_immediate_data_params_iter
            .into_iter()
            .map(|imm_data| self.post_send_immediate_data(peer_rank_id, *imm_data))
            .collect()
    }
}

pub trait RdmaReceiveImmediateDataTransportNetworkNode {
    type WorkRequest: RdmaWorkRequest;
    type PostError: Error;

    fn post_receive_immediate_data(
        &mut self,
        peer_rank_id: usize,
    ) -> Result<Self::WorkRequest, Self::PostError>;

    fn post_receive_immediate_data_batch(
        &mut self,
        peer_rank_id: usize,
        num_receives: usize,
    ) -> Vec<Result<Self::WorkRequest, Self::PostError>> {
        (0..num_receives)
            .into_iter()
            .map(|_| self.post_receive_immediate_data(peer_rank_id))
            .collect()
    }
}

#[derive(Debug, Clone)]
pub enum RdmaNamedMemory {
    Normal {
        id: String,
        ptr: *mut u8,
        length: usize,
    },
    HugeTlb {
        id: String,
        ptr: *mut u8,
        length: usize,
    },
    Dma {
        id: String,
        fd: i32,
        length: usize,
    },
}

impl RdmaNamedMemory {
    pub fn new(id: impl Into<String>, ptr: *mut u8, length: usize) -> Self {
        Self::Normal {
            id: id.into(),
            ptr,
            length,
        }
    }

    pub fn new_hugetlb(id: impl Into<String>, ptr: *mut u8, length: usize) -> Self {
        Self::HugeTlb {
            id: id.into(),
            ptr,
            length,
        }
    }

    pub fn new_dma(id: impl Into<String>, fd: i32, length: usize) -> Self {
        Self::Dma {
            id: id.into(),
            fd,
            length,
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Normal { id, .. } => id,
            Self::HugeTlb { id, .. } => id,
            Self::Dma { id, .. } => id,
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

#[derive(Debug, Error)]
#[error("Non matching memory region count, expected {expected}, got {got}")]
pub struct NonMatchingMemoryRegionCount {
    pub(super) expected: usize,
    pub(super) got: usize,
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

    fn connection_mut(
        &mut self,
        idx: usize,
    ) -> Option<RdmaNetworkSelfGroupConnection<Self::Connection>>;
}

pub enum RdmaNetworkSelfGroupConnection<'network, Conn> {
    SelfConnection,
    PeerConnection(usize, &'network mut Conn),
}
