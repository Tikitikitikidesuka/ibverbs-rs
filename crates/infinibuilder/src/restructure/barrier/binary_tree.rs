use crate::restructure::barrier::{RdmaNetworkBarrier, RdmaNetworkMemoryRegionComponent};
use crate::restructure::rdma_connection::{RdmaConnection, RdmaWorkRequest};
use crate::restructure::rdma_network_node::{
    RdmaNetworkSelfGroupConnection, RdmaNetworkSelfGroupConnections,
};
use crate::restructure::spin_poll::spin_poll_batched;
use std::ptr::{read_volatile, write_volatile};
use std::time::Duration;
use thiserror::Error;
use PeerRole::*;

#[derive(Debug, Error)]
pub enum RdmaNetworkBinaryTreeBarrierError {
    #[error("Centralized barrier timeout: {0}:")]
    Timeout(String),
    #[error("Centralized barrier RDMA error: {0}")]
    RdmaError(String),
}

pub struct RdmaNetworkUnregisteredBinaryTreeBarrier {
    memory: Vec<u8>,
}

#[derive(Debug)]
pub struct RdmaNetworkBinaryTreeBarrier<MR, RMR> {
    memory: Vec<u8>,
    mrs: Vec<(MR, RMR)>,
}

impl RdmaNetworkBinaryTreeBarrier<(), ()> {
    pub fn new() -> RdmaNetworkUnregisteredBinaryTreeBarrier {
        RdmaNetworkUnregisteredBinaryTreeBarrier { memory: vec![] }
    }
}

/// Two bytes per connection, first is local flag that never changes to send ready.
/// The second is the remote flag to write by the remote peer.

const BYTES_PER_CONNECTION: usize = 2;
const NOT_READY_FLAG: u8 = 0b01010101;
const READY_FLAG: u8 = 0b10101010;

fn setup_memory() -> Vec<u8> {
    // Create a vector wit 3 * BYTES_PER_CONNECTION bytes (parent and two children).
    // Even bytes, representing local flags, all set to READY.
    // Odd bytes, representing remote peer flags, all set to NOT_READY.
    // First two bytes represent parent.
    // The next are left and right children.
    (0..(3 * BYTES_PER_CONNECTION))
        .into_iter()
        .map(|byte_idx| match byte_idx % 2 == 0 {
            true => READY_FLAG,
            false => NOT_READY_FLAG,
        })
        .collect()
}

impl RdmaNetworkUnregisteredBinaryTreeBarrier {
    fn peer_memory(&mut self, peer: PeerRole) -> (*mut u8, usize) {
        let ptr = &mut self.memory[peer.idx() * BYTES_PER_CONNECTION] as *mut u8;
        (ptr, BYTES_PER_CONNECTION)
    }
}

#[derive(Debug, Copy, Clone)]
enum PeerRole {
    Parent,
    LeftChild,
    RightChild,
}

impl PeerRole {
    pub fn idx(&self) -> usize {
        match self {
            Parent => 0,
            LeftChild => 1,
            RightChild => 2,
        }
    }
}

impl<MR, RMR> RdmaNetworkBinaryTreeBarrier<MR, RMR> {
    fn peer_group_idx(&self, idx: usize, group_size: usize, peer: PeerRole) -> Option<usize> {
        match peer {
            PeerRole::Parent => {
                if idx == 0 {
                    None
                } else {
                    Some((idx - 1) / 2)
                }
            }
            PeerRole::LeftChild => {
                let group_idx = idx * 2 + 1;
                if group_idx >= group_size {
                    None
                } else {
                    Some(group_idx)
                }
            }
            PeerRole::RightChild => {
                let group_idx = idx * 2 + 2;
                if group_idx >= group_size {
                    None
                } else {
                    Some(group_idx)
                }
            }
        }
    }

    fn peer_mr(&self, peer: PeerRole) -> &(MR, RMR) {
        &self.mrs[peer.idx()]
    }

    fn reset_remote_peer_flag(&mut self, peer: PeerRole) {
        let ptr = &mut self.memory[peer.idx() * BYTES_PER_CONNECTION + 1] as *mut u8;
        unsafe { write_volatile(ptr, NOT_READY_FLAG) };
    }

    fn read_remote_peer_flag(&self, peer: PeerRole) -> u8 {
        let ptr = &self.memory[peer.idx() * BYTES_PER_CONNECTION + 1] as *const u8;
        unsafe { read_volatile(ptr) }
    }
}

#[derive(Debug, Error)]
#[error("Non matching memory region count, expected {expected}, got {got}")]
pub struct NonMatchingMemoryRegionCount {
    expected: usize,
    got: usize,
}

impl<MR, RMR> RdmaNetworkMemoryRegionComponent<MR, RMR>
    for RdmaNetworkUnregisteredBinaryTreeBarrier
{
    type Registered = RdmaNetworkBinaryTreeBarrier<MR, RMR>;
    type RegisterError = NonMatchingMemoryRegionCount;

    fn memory(&mut self, _num_connections: usize) -> Vec<(*mut u8, usize)> {
        self.memory = setup_memory();
        vec![
            self.peer_memory(Parent),
            self.peer_memory(LeftChild),
            self.peer_memory(RightChild),
        ]
    }

    fn registered_mrs(self, mrs: Vec<(MR, RMR)>) -> Result<Self::Registered, Self::RegisterError> {
        if mrs.len() != 3 {
            return Err(NonMatchingMemoryRegionCount {
                expected: 3,
                got: mrs.len(),
            });
        }

        Ok(RdmaNetworkBinaryTreeBarrier {
            memory: self.memory,
            mrs,
        })
    }
}

impl<MR, RemoteMR> RdmaNetworkBarrier<MR, RemoteMR> for RdmaNetworkBinaryTreeBarrier<MR, RemoteMR> {
    type Error = RdmaNetworkBinaryTreeBarrierError;

    fn barrier<
        'network,
        Conn: RdmaConnection<MR = MR, RemoteMR = RemoteMR> + 'network,
        GroupConns: RdmaNetworkSelfGroupConnections<'network, Conn>,
    >(
        &mut self,
        connections: GroupConns,
        timeout: Duration,
    ) -> Result<(), Self::Error> {
        let idx = connections.self_idx();

        self.binary_tree_barrier(connections, timeout)
    }
}

impl<MR, RemoteMR> RdmaNetworkBinaryTreeBarrier<MR, RemoteMR> {
    fn binary_tree_barrier<
        'network,
        Conn: RdmaConnection<MR = MR, RemoteMR = RemoteMR> + 'network,
        GroupConns: RdmaNetworkSelfGroupConnections<'network, Conn>,
    >(
        &mut self,
        mut connections: GroupConns,
        timeout: Duration,
    ) -> Result<(), RdmaNetworkBinaryTreeBarrierError> {
        let mut available_time = timeout;

        // 1. Wait for the two children
        available_time -= self.wait_for_peer(LeftChild, &connections, available_time)?;
        available_time -= self.wait_for_peer(RightChild, &connections, available_time)?;

        // 2. Notify parent
        available_time -= self.notify_peer(Parent, &mut connections, available_time)?;

        // 3. Wait for parent
        available_time -= self.wait_for_peer(Parent, &connections, available_time)?;

        // 4. Notify two children
        available_time -= self.notify_peer(LeftChild, &mut connections, available_time)?;
        available_time -= self.notify_peer(RightChild, &mut connections, available_time)?;

        Ok(())
    }

    fn wait_for_peer<
        'network,
        Conn: RdmaConnection<MR = MR, RemoteMR = RemoteMR> + 'network,
        GroupConns: RdmaNetworkSelfGroupConnections<'network, Conn>,
    >(
        &mut self,
        peer: PeerRole,
        connections: &GroupConns,
        timeout: Duration,
    ) -> Result<Duration, RdmaNetworkBinaryTreeBarrierError> {
        Ok(
            if let Some(_group_idx) =
                self.peer_group_idx(connections.self_idx(), connections.len(), peer)
            {
                let (_, elapsed) = spin_poll_batched(
                    || (self.read_remote_peer_flag(peer) == READY_FLAG).then_some(()),
                    timeout,
                    1024,
                )
                .map_err(|_| {
                    RdmaNetworkBinaryTreeBarrierError::Timeout(format!(
                        "Timeout waiting for {peer:?} notification"
                    ))
                })?;
                self.reset_remote_peer_flag(peer);
                elapsed
            } else {
                Duration::from_nanos(0)
            },
        )
    }

    fn notify_peer<
        'network,
        Conn: RdmaConnection<MR = MR, RemoteMR = RemoteMR> + 'network,
        GroupConns: RdmaNetworkSelfGroupConnections<'network, Conn>,
    >(
        &mut self,
        peer: PeerRole,
        connections: &mut GroupConns,
        timeout: Duration,
    ) -> Result<Duration, RdmaNetworkBinaryTreeBarrierError> {
        Ok(
            if let Some(group_idx) =
                self.peer_group_idx(connections.self_idx(), connections.len(), peer)
            {
                let peer_conn = connections.connection_mut(group_idx);
                let (peer_mr, peer_remote_mr) = self.peer_mr(peer);
                if let Some(RdmaNetworkSelfGroupConnection::PeerConnection(_rank_id, conn)) =
                    peer_conn
                {
                    let mut wr = conn
                        .post_write(peer_mr, 0..=0, peer_remote_mr, 1..=1, None)
                        .map_err(|error| {
                            RdmaNetworkBinaryTreeBarrierError::RdmaError(format!(
                                "Error issuing RDMA write to {peer:?}: {error}"
                            ))
                        })?;
                    let (_, elapsed) = wr.spin_poll_batched(timeout, 1024).map_err(|error| {
                        RdmaNetworkBinaryTreeBarrierError::RdmaError(format!(
                            "Error during RDMA write to left child: {error}"
                        ))
                    })?;
                    elapsed
                } else {
                    Duration::from_nanos(0)
                }
            } else {
                Duration::from_nanos(0)
            },
        )
    }
}
