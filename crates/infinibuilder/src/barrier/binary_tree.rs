use crate::barrier::{
    MrPair, NonMatchingMemoryRegionCount, RdmaNetworkBarrier, RdmaNetworkBarrierError,
    RdmaNetworkMemoryRegionComponent,
};
use crate::rdma_connection::{RdmaConnection, RdmaWorkRequest};
use crate::rdma_network_node::{RdmaNetworkSelfGroupConnection, RdmaNetworkSelfGroupConnections};
use crate::spin_poll::spin_poll_batched;
use PeerRole::*;
use std::ptr::{read_volatile, write_volatile};
use std::time::Duration;

#[derive(Debug)]
pub struct UnregisteredBinaryTreeBarrier {
    memory: Vec<u8>,
}

#[derive(Debug)]
pub struct BinaryTreeBarrier {
    memory: Vec<u8>,
    mrs: Vec<MrPair>,
}

impl BinaryTreeBarrier {
    pub fn new() -> UnregisteredBinaryTreeBarrier {
        UnregisteredBinaryTreeBarrier { memory: vec![] }
    }
}

/// Two bytes per connection, first is local flag that never changes to send ready.
/// The second is the remote flag to write by the remote peer.

const BYTES_PER_CONNECTION: usize = 2;
const NOT_READY_FLAG: u8 = 0b01010101;
const READY_FLAG: u8 = 0b10101010;

fn setup_memory(num_connections: usize) -> Vec<u8> {
    // Create a vector wit num_connections * BYTES_PER_CONNECTION bytes.
    // Even bytes, representing local flags, all set to READY.
    // Odd bytes, representing remote peer flags, all set to NOT_READY.
    (0..(num_connections * BYTES_PER_CONNECTION))
        .into_iter()
        .map(|byte_idx| match byte_idx % 2 == 0 {
            true => READY_FLAG,
            false => NOT_READY_FLAG,
        })
        .collect()
}

impl UnregisteredBinaryTreeBarrier {
    fn memory_of_connection(&mut self, rank_id: usize) -> (*mut u8, usize) {
        let ptr = &mut self.memory[rank_id * BYTES_PER_CONNECTION] as *mut u8;
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

impl BinaryTreeBarrier {
    fn peer_group_idx(&self, idx: usize, group_size: usize, peer: PeerRole) -> Option<usize> {
        match peer {
            Parent => {
                if idx == 0 {
                    None
                } else {
                    Some((idx - 1) / 2)
                }
            }
            LeftChild => {
                let group_idx = idx * 2 + 1;
                if group_idx >= group_size {
                    None
                } else {
                    Some(group_idx)
                }
            }
            RightChild => {
                let group_idx = idx * 2 + 2;
                if group_idx >= group_size {
                    None
                } else {
                    Some(group_idx)
                }
            }
        }
    }

    fn connection_mr(&self, rank_id: usize) -> &MrPair {
        &self.mrs[rank_id]
    }

    fn read_remote_peer_flag(&self, rank_id: usize) -> u8 {
        let ptr = &self.memory[rank_id * BYTES_PER_CONNECTION + 1] as *const u8;
        unsafe { read_volatile(ptr) }
    }

    fn reset_remote_peer_flag(&mut self, rank_id: usize) {
        let ptr = &mut self.memory[rank_id * BYTES_PER_CONNECTION + 1] as *mut u8;
        unsafe { write_volatile(ptr, NOT_READY_FLAG) };
    }
}

impl RdmaNetworkMemoryRegionComponent for UnregisteredBinaryTreeBarrier {
    type Registered = BinaryTreeBarrier;
    type RegisterError = NonMatchingMemoryRegionCount;

    fn memory(&mut self, num_connections: usize) -> Vec<(*mut u8, usize)> {
        self.memory = setup_memory(num_connections);
        (0..num_connections)
            .into_iter()
            .map(|conn_idx| self.memory_of_connection(conn_idx))
            .collect()
    }

    fn registered_mrs(self, mrs: Vec<MrPair>) -> Result<Self::Registered, Self::RegisterError> {
        let num_connections = self.memory.len() / BYTES_PER_CONNECTION;
        if mrs.len() != num_connections {
            return Err(NonMatchingMemoryRegionCount {
                expected: num_connections,
                got: mrs.len(),
            });
        }

        Ok(BinaryTreeBarrier {
            memory: self.memory,
            mrs,
        })
    }
}

impl RdmaNetworkBarrier for BinaryTreeBarrier {
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
        let idx = connections.self_idx();

        self.binary_tree_barrier(connections, timeout)
    }
}

impl BinaryTreeBarrier {
    fn binary_tree_barrier<
        'network,
        Conn: RdmaConnection + 'network,
        GroupConns: RdmaNetworkSelfGroupConnections<'network, Conn>,
    >(
        &mut self,
        mut connections: GroupConns,
        timeout: Duration,
    ) -> Result<(), RdmaNetworkBarrierError> {
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
        Conn: RdmaConnection + 'network,
        GroupConns: RdmaNetworkSelfGroupConnections<'network, Conn>,
    >(
        &mut self,
        peer: PeerRole,
        connections: &GroupConns,
        timeout: Duration,
    ) -> Result<Duration, RdmaNetworkBarrierError> {
        Ok(
            if let Some(group_idx) =
                self.peer_group_idx(connections.self_idx(), connections.len(), peer)
            {
                let peer_rank_id = connections.rank_id(group_idx).unwrap();
                let (_, elapsed) = spin_poll_batched(
                    || (self.read_remote_peer_flag(peer_rank_id) == READY_FLAG).then_some(()),
                    timeout,
                    1024,
                )
                .map_err(|_| {
                    RdmaNetworkBarrierError::Timeout(format!(
                        "Timeout waiting for {peer:?} notification"
                    ))
                })?;
                self.reset_remote_peer_flag(peer_rank_id);
                elapsed
            } else {
                Duration::from_nanos(0)
            },
        )
    }

    fn notify_peer<
        'network,
        Conn: RdmaConnection + 'network,
        GroupConns: RdmaNetworkSelfGroupConnections<'network, Conn>,
    >(
        &mut self,
        peer: PeerRole,
        connections: &mut GroupConns,
        timeout: Duration,
    ) -> Result<Duration, RdmaNetworkBarrierError> {
        Ok(
            if let Some(group_idx) =
                self.peer_group_idx(connections.self_idx(), connections.len(), peer)
            {
                let peer_rank_id = connections.rank_id(group_idx).unwrap();
                let peer_conn = connections.connection_mut(group_idx);
                let peer_mr_pair = self.connection_mr(peer_rank_id);
                if let Some(RdmaNetworkSelfGroupConnection::PeerConnection(_rank_id, conn)) =
                    peer_conn
                {
                    let mut wr = conn
                        .post_write(
                            peer_mr_pair.local_mr,
                            0..=0,
                            peer_mr_pair.remote_mr,
                            1..=1,
                            None,
                        )
                        .map_err(|error| {
                            RdmaNetworkBarrierError::RdmaError(format!(
                                "Error issuing RDMA write to {peer:?}: {error}"
                            ))
                        })?;
                    let (_, elapsed) = wr.spin_poll_batched(timeout, 1024).map_err(|error| {
                        RdmaNetworkBarrierError::RdmaError(format!(
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
