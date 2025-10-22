use crate::restructure::barrier::{RdmaNetworkBarrier, RdmaNetworkMemoryRegionComponent};
use crate::restructure::rdma_connection::{
    RdmaConnection, RdmaWorkRequest, WorkRequestSpinPollError,
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

fn left_child_idx(idx: usize) -> usize {
    idx * 2 + 1
}

fn right_child_idx(idx: usize) -> usize {
    left_child_idx(idx) + 1
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
        mut connections: GroupConns,
        timeout: Duration,
    ) -> Result<(), Self::Error> {
        let idx = connections.self_idx();

        if idx == 0 {
            // Root
            self.root_barrier(connections, timeout)
        } else {
            // Non root
            self.non_root_barrier(connections, timeout)
        }
    }
}

impl<MR, RemoteMR> RdmaNetworkBinaryTreeBarrier<MR, RemoteMR> {
    fn root_barrier<
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
        let (_, elapsed) = spin_poll_batched(
            || (read_remote_left_child_flag(self.memory.as_slice()) == READY_FLAG).then_some(()),
            available_time,
            1024,
        )
        .map_err(|_| {
            RdmaNetworkBinaryTreeBarrierError::Timeout(
                "Timeout waiting for left children notification".to_string(),
            )
        })?;
        available_time -= elapsed;
        let (_, elapsed) = spin_poll_batched(
            || (read_remote_right_child_flag(self.memory.as_slice()) == READY_FLAG).then_some(()),
            available_time,
            1024,
        )
        .map_err(|_| {
            RdmaNetworkBinaryTreeBarrierError::Timeout(
                "Timeout waiting for left children notification".to_string(),
            )
        })?;
        available_time -= elapsed;

        // 2. Notify two children
        let mut left_child_conn = connections
            .connection_mut(left_child_idx(connections.self_idx()));
        if let Some(RdmaNetworkSelfGroupConnection::PeerConnection(rank_id, conn)) = left_child_conn {
            let mut wr = conn.post_write()
        }


        for idx in 0..connections.len() {
            if let RdmaNetworkSelfGroupConnection::PeerConnection(rank_id, peer_conn) =
                connections.connection_mut(idx).unwrap()
            {
                println!("Notifying peer {rank_id} as central barrier coordinator");

                let (mr, rmr) = &self.mrs[rank_id];
                let mut wr = peer_conn.post_write(mr, 0..=0, rmr, 1..=1, None).unwrap(); // TODO: BETTER HANDLING
                let (wc, elapsed) =
                    wr.spin_poll_batched(timeout, 1024)
                        .map_err(|error| match error {
                            WorkRequestSpinPollError::Timeout(_) => {
                                RdmaNetworkBinaryTreeBarrierError::Timeout(
                                    "Timeout trying to notify coordinated".to_string(),
                                )
                            }
                            WorkRequestSpinPollError::ExecutionError(error) => {
                                RdmaNetworkBinaryTreeBarrierError::RdmaError(format!(
                                    "RDMA error trying to notify coordinated: {error}"
                                ))
                            }
                        })?;
                available_time -= elapsed;
            }
        }

        Ok(())
    }

    fn non_root_barrier<
        'network,
        Conn: RdmaConnection<MR = MR, RemoteMR = RemoteMR> + 'network,
        GroupConns: RdmaNetworkSelfGroupConnections<'network, Conn>,
    >(
        &mut self,
        mut connections: GroupConns,
        timeout: Duration,
    ) -> Result<(), RdmaNetworkBinaryTreeBarrierError> {
        println!("Connecting to coordinator");

        // Groups can never be empty
        if let RdmaNetworkSelfGroupConnection::PeerConnection(rank_id, master_conn) =
            connections.connection_mut(0).unwrap()
        {
            let (mr, rmr) = &self.mrs[rank_id];

            println!("Notifying coordinator at {rank_id}");
            let mut wr = master_conn.post_write(mr, 0..=0, rmr, 1..=1, None).unwrap(); // TODO: BETTER HANDLING
            let (wc, elapsed) =
                wr.spin_poll_batched(timeout, 1024)
                    .map_err(|error| match error {
                        WorkRequestSpinPollError::Timeout(_) => {
                            RdmaNetworkBinaryTreeBarrierError::Timeout(
                                "Timeout trying to notify coordinator".to_string(),
                            )
                        }
                        WorkRequestSpinPollError::ExecutionError(error) => {
                            RdmaNetworkBinaryTreeBarrierError::RdmaError(format!(
                                "RDMA error trying to notify coordinator: {error}"
                            ))
                        }
                    })?;

            println!("Waiting for coordinator notification");
            // Wait until remote is ready
            spin_poll_batched(
                || {
                    (read_remote_peer_flag(self.memory.as_slice(), rank_id) == READY_FLAG)
                        .then_some(())
                },
                timeout - elapsed,
                1024,
            )
            .map_err(|error| {
                RdmaNetworkBinaryTreeBarrierError::Timeout(
                    "Timeout waiting for coordinator notification".to_string(),
                )
            })?;

            // Reset remote peer flag
            reset_remote_peer_flag(self.memory.as_mut_slice(), rank_id);
        } else {
            panic!(
                "Coordinator of centralized sync must always be group index zero. \
                     Check group construction is being handled properly"
            );
        }

        Ok(())
    }
}
