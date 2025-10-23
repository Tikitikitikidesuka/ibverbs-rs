use crate::restructure::barrier::{RdmaNetworkBarrier, RdmaNetworkMemoryRegionComponent};
use crate::restructure::rdma_connection::{
    RdmaConnection, RdmaWorkRequest,
};
use crate::restructure::rdma_network_node::{
    RdmaNetworkSelfGroupConnection, RdmaNetworkSelfGroupConnections,
};
use crate::restructure::spin_poll::spin_poll_batched;
use std::ptr::{read_volatile, write_volatile};
use std::time::Duration;
use thiserror::Error;

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

fn memory_of_parent(memory: &mut [u8]) -> (*mut u8, usize) {
    memory_of_connection(memory, 0)
}

fn memory_of_left_child(memory: &mut [u8]) -> (*mut u8, usize) {
    memory_of_connection(memory, 1)
}

fn memory_of_right_child(memory: &mut [u8]) -> (*mut u8, usize) {
    memory_of_connection(memory, 2)
}

fn memory_of_connection(memory: &mut [u8], rank_id: usize) -> (*mut u8, usize) {
    let ptr = &mut memory[rank_id * BYTES_PER_CONNECTION] as *mut u8;
    (ptr, BYTES_PER_CONNECTION)
}

fn read_remote_parent_flag(memory: &[u8]) -> u8 {
    read_remote_peer_flag(memory, 0)
}

fn read_remote_left_child_flag(memory: &[u8]) -> u8 {
    read_remote_peer_flag(memory, 1)
}

fn read_remote_right_child_flag(memory: &[u8]) -> u8 {
    read_remote_peer_flag(memory, 2)
}

fn read_remote_peer_flag(memory: &[u8], rank_id: usize) -> u8 {
    let ptr = &memory[rank_id * BYTES_PER_CONNECTION + 1] as *const u8;
    unsafe { read_volatile(ptr) }
}

fn reset_remote_parent_flag(memory: &mut [u8]) {
    reset_remote_peer_flag(memory, 0);
}

fn reset_remote_left_child_flag(memory: &mut [u8]) {
    reset_remote_peer_flag(memory, 1);
}

fn reset_remote_right_child_flag(memory: &mut [u8]) {
    reset_remote_peer_flag(memory, 2);
}

fn reset_remote_peer_flag(memory: &mut [u8], rank_id: usize) {
    let ptr = &mut memory[rank_id * BYTES_PER_CONNECTION + 1] as *mut u8;
    unsafe { write_volatile(ptr, NOT_READY_FLAG) };
}

impl<MR, RMR> RdmaNetworkBinaryTreeBarrier<MR, RMR> {
    fn parent_idx(&self, idx: usize) -> Option<usize> {
        if idx == 0 { None } else { Some((idx - 1) / 2) }
    }

    fn left_child_idx(&self, idx: usize, group_size: usize) -> Option<usize> {
        let lci = idx * 2 + 1;
        if lci >= group_size {
            None
        } else {
            Some(lci)
        }
    }

    fn right_child_idx(&self, idx: usize, group_size: usize) -> Option<usize> {
        let rci = idx * 2 + 2;
        if rci >= group_size {
            None
        } else {
            Some(rci)
        }
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

    fn memory(&mut self, num_connections: usize) -> Vec<(*mut u8, usize)> {
        self.memory = setup_memory();
        (0..3)
            .into_iter()
            .map(|conn_idx| memory_of_connection(self.memory.as_mut_slice(), conn_idx))
            .collect()
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
        if let Some(lci) = self.left_child_idx(connections.self_idx(), connections.len()) {
            println!("Waiting for left child");
            let (_, elapsed) = spin_poll_batched(
                || {
                    (read_remote_left_child_flag(self.memory.as_slice()) == READY_FLAG)
                        .then_some(())
                },
                available_time,
                1024,
            )
            .map_err(|_| {
                RdmaNetworkBinaryTreeBarrierError::Timeout(
                    "Timeout waiting for left children notification".to_string(),
                )
            })?;
            reset_remote_left_child_flag(self.memory.as_mut_slice());
            available_time -= elapsed;
        }
        if let Some(rci) = self.right_child_idx(connections.self_idx(), connections.len()) {
            println!("Waiting for right child");
            let (_, elapsed) = spin_poll_batched(
                || {
                    (read_remote_right_child_flag(self.memory.as_slice()) == READY_FLAG)
                        .then_some(())
                },
                available_time,
                1024,
            )
            .map_err(|_| {
                RdmaNetworkBinaryTreeBarrierError::Timeout(
                    "Timeout waiting for left children notification".to_string(),
                )
            })?;
            reset_remote_right_child_flag(self.memory.as_mut_slice());
            available_time -= elapsed;
        }

        // 2. Notify parent
        if let Some(pi) = self.parent_idx(connections.self_idx()) {
            println!("Notifying parent");
            let parent_conn = connections.connection_mut(pi);
            let (parent_mr, parent_remote_mr) = &self.mrs[0];
            if let Some(RdmaNetworkSelfGroupConnection::PeerConnection(rank_id, conn)) = parent_conn
            {
                let mut wr = conn
                    .post_write(parent_mr, 0..=0, parent_remote_mr, 1..=1, None)
                    .map_err(|error| {
                        RdmaNetworkBinaryTreeBarrierError::RdmaError(format!(
                            "Error issuing RDMA write to left child: {error}"
                        ))
                    })?;
                let (_, elapsed) = wr
                    .spin_poll_batched(available_time, 1024)
                    .map_err(|error| {
                        RdmaNetworkBinaryTreeBarrierError::RdmaError(format!(
                            "Error during RDMA write to left child: {error}"
                        ))
                    })?;
                available_time -= elapsed;
            }

            // 3. Wait for parent
            println!("Waiting for parent");
            let (_, elapsed) = spin_poll_batched(
                || (read_remote_parent_flag(self.memory.as_slice()) == READY_FLAG).then_some(()),
                available_time,
                1024,
            )
            .map_err(|_| {
                RdmaNetworkBinaryTreeBarrierError::Timeout(
                    "Timeout waiting for left children notification".to_string(),
                )
            })?;
            reset_remote_parent_flag(self.memory.as_mut_slice());
            available_time -= elapsed;
        }

        // 4. Notify two children
        if let Some(lci) = self.left_child_idx(connections.self_idx(), connections.len()) {
            println!("Notifying left child");
            let left_child_conn = connections.connection_mut(lci);
            let (left_child_mr, left_child_remote_mr) = &self.mrs[1];
            if let Some(RdmaNetworkSelfGroupConnection::PeerConnection(rank_id, conn)) =
                left_child_conn
            {
                let mut wr = conn
                    .post_write(left_child_mr, 0..=0, left_child_remote_mr, 1..=1, None)
                    .map_err(|error| {
                        RdmaNetworkBinaryTreeBarrierError::RdmaError(format!(
                            "Error issuing RDMA write to left child: {error}"
                        ))
                    })?;
                let (_, elapsed) = wr
                    .spin_poll_batched(available_time, 1024)
                    .map_err(|error| {
                        RdmaNetworkBinaryTreeBarrierError::RdmaError(format!(
                            "Error during RDMA write to left child: {error}"
                        ))
                    })?;
                available_time -= elapsed;
            }
        }
        if let Some(rci) = self.right_child_idx(connections.self_idx(), connections.len()) {
            println!("Notifying right child");
            let right_child_conn = connections.connection_mut(rci);
            let (right_child_mr, right_child_remote_mr) = &self.mrs[2];
            if let Some(RdmaNetworkSelfGroupConnection::PeerConnection(rank_id, conn)) =
                right_child_conn
            {
                let mut wr = conn
                    .post_write(right_child_mr, 0..=0, right_child_remote_mr, 1..=1, None)
                    .map_err(|error| {
                        RdmaNetworkBinaryTreeBarrierError::RdmaError(format!(
                            "Error issuing RDMA write to left child: {error}"
                        ))
                    })?;
                let (_, elapsed) = wr
                    .spin_poll_batched(available_time, 1024)
                    .map_err(|error| {
                        RdmaNetworkBinaryTreeBarrierError::RdmaError(format!(
                            "Error during RDMA write to left child: {error}"
                        ))
                    })?;
                available_time -= elapsed;
            }
        }

        Ok(())
    }
}
