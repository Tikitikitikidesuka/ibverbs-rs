use crate::barrier::{MrPair, NonMatchingMemoryRegionCount, RdmaNetworkBarrier, RdmaNetworkBarrierError, RdmaNetworkMemoryRegionComponent};
use crate::rdma_connection::{RdmaConnection, RdmaWorkRequest};
use crate::rdma_network_node::{
    RdmaNetworkSelfGroupConnection, RdmaNetworkSelfGroupConnections,
};
use crate::spin_poll::spin_poll_batched;
use Direction::*;
use std::ops::RangeBounds;
use std::ptr::{read_volatile, write_volatile};
use std::time::Duration;

#[derive(Debug)]
pub struct UnregisteredDisseminationBarrier {
    memory: Vec<u8>,
}

#[derive(Debug)]
pub struct DisseminationBarrier {
    memory: Vec<u8>,
    mrs: Vec<MrPair>,
}


impl DisseminationBarrier {
    pub fn new() -> UnregisteredDisseminationBarrier {
        UnregisteredDisseminationBarrier { memory: vec![] }
    }
}

/// Four bytes per connection:
/// The first is the local flag that never changes to send ready.
/// The second is the parity counter, it counts how many barriers have passed through this mr.
/// The second is the remote flag with even parity to write by the remote peer.
/// The third is the remote flag with odd parity to write by the remote peer.

const BYTES_PER_CONNECTION: usize = 4;
const NOT_READY_FLAG: u8 = 0b01010101;
const READY_FLAG: u8 = 0b10101010;

fn setup_memory(num_connections: usize) -> Vec<u8> {
    // Create a vector wit num_connections * BYTES_PER_CONNECTION(4) bytes.
    // Following this pattern:
    // - first byte = READY,
    // - second byte = 0u8,
    // - third byte = NOT_READY,
    // - fourth byte = NOT_READY
    (0..(num_connections * BYTES_PER_CONNECTION))
        .map(|byte_idx| match byte_idx % BYTES_PER_CONNECTION {
            0 => READY_FLAG,     // Local flag
            1 => 0,              // Parity counter
            _ => NOT_READY_FLAG, // Remote flags (bytes 2 and 3)
        })
        .collect()
}

impl UnregisteredDisseminationBarrier {
    fn memory_of_connection(&mut self, rank_id: usize) -> (*mut u8, usize) {
        let ptr = &mut self.memory[rank_id * BYTES_PER_CONNECTION] as *mut u8;
        (ptr, BYTES_PER_CONNECTION)
    }
}

impl DisseminationBarrier {
    fn connection_mr(&self, rank_id: usize) -> &MrPair {
        &self.mrs[rank_id]
    }

    fn parity_counter_idx(&self, rank_id: usize) -> usize {
        rank_id * BYTES_PER_CONNECTION + 1
    }

    fn parity_counter(&self, rank_id: usize) -> u8 {
        self.memory[self.parity_counter_idx(rank_id)]
    }

    fn remote_peer_flag_mr_range(&self, rank_id: usize) -> impl RangeBounds<usize> {
        let idx = (2 + self.parity_counter(rank_id) % 2) as usize;
        idx..=idx
    }

    fn remote_peer_flag_idx(&self, rank_id: usize) -> usize {
        let parity_cntr_idx = self.parity_counter_idx(rank_id);
        let parity = self.memory[parity_cntr_idx] % 2;
        parity_cntr_idx + 1 + parity as usize
    }

    fn advance_parity_counter(&mut self, rank_id: usize) {
        let parity_cntr_idx = self.parity_counter_idx(rank_id);
        self.memory[parity_cntr_idx] = (self.memory[parity_cntr_idx] + 1) % 2;
    }

    fn read_remote_peer_flag(&self, rank_id: usize) -> u8 {
        let flag_idx = self.remote_peer_flag_idx(rank_id);
        let ptr = &self.memory[flag_idx] as *const u8;
        unsafe { read_volatile(ptr) }
    }

    fn reset_remote_peer_flag(&mut self, rank_id: usize) {
        let flag_idx = self.remote_peer_flag_idx(rank_id);
        let ptr = &mut self.memory[flag_idx] as *mut u8;
        unsafe { write_volatile(ptr, NOT_READY_FLAG) };
    }
}

impl RdmaNetworkMemoryRegionComponent for UnregisteredDisseminationBarrier {
    type Registered = DisseminationBarrier;
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

        Ok(DisseminationBarrier {
            memory: self.memory,
            mrs,
        })
    }
}

impl RdmaNetworkBarrier for DisseminationBarrier {
    type Error = RdmaNetworkBarrierError;

    fn barrier<
        'network,
        Conn: RdmaConnection + 'network,
        GroupConns: RdmaNetworkSelfGroupConnections<'network, Conn>,
    >(
        &mut self,
        mut connections: GroupConns,
        timeout: Duration,
    ) -> Result<(), Self::Error> {
        let mut distance = 1;
        let max_distance = connections.len();
        while distance < max_distance {
            self.barrier_round(distance, &mut connections, timeout)?;
            distance *= 2;
        }

        Ok(())
    }
}

#[derive(Debug, Copy, Clone)]
enum Direction {
    Right,
    Left,
}

impl DisseminationBarrier {
    fn barrier_round<
        'network,
        Conn: RdmaConnection + 'network,
        GroupConns: RdmaNetworkSelfGroupConnections<'network, Conn>,
    >(
        &mut self,
        distance: usize,
        connections: &mut GroupConns,
        timeout: Duration,
    ) -> Result<Duration, RdmaNetworkBarrierError> {
        let mut available_time = timeout;

        let right_idx = self.distance_idx(connections, distance, Right);
        let left_idx = self.distance_idx(connections, distance, Left);

        available_time -= self.notify_peer(
            self.distance_idx(connections, distance, Right),
            connections,
            timeout,
        )?;
        available_time -= self.wait_for_peer(
            self.distance_idx(connections, distance, Left),
            connections,
            timeout,
        )?;

        // Advance parity after both operations complete
        let right_rank = connections.rank_id(right_idx).unwrap();
        let left_rank = connections.rank_id(left_idx).unwrap();

        self.advance_parity_counter(right_rank);
        if left_rank != right_rank {
            // Only advance twice if communicating with different peers
            self.advance_parity_counter(left_rank);
        }
        Ok(available_time)
    }

    fn distance_idx<
        'network,
        Conn: RdmaConnection + 'network,
        GroupConns: RdmaNetworkSelfGroupConnections<'network, Conn>,
    >(
        &self,
        connections: &GroupConns,
        distance: usize,
        direction: Direction,
    ) -> usize {
        let len = connections.len();
        let distance = match direction {
            Right => distance % len,
            Left => len - (distance % len),
        };
        (connections.self_idx() + distance) % len
    }

    fn wait_for_peer<
        'network,
        Conn: RdmaConnection + 'network,
        GroupConns: RdmaNetworkSelfGroupConnections<'network, Conn>,
    >(
        &mut self,
        idx: usize,
        connections: &GroupConns,
        timeout: Duration,
    ) -> Result<Duration, RdmaNetworkBarrierError> {
        let peer_rank_id = connections.rank_id(idx).unwrap();
        let (_, elapsed) = spin_poll_batched(
            || (self.read_remote_peer_flag(peer_rank_id) == READY_FLAG).then_some(()),
            timeout,
            1024,
        )
        .map_err(|_| {
            RdmaNetworkBarrierError::Timeout(format!(
                "Timeout waiting for {peer_rank_id:?} notification"
            ))
        })?;
        self.reset_remote_peer_flag(peer_rank_id);
        Ok(elapsed)
    }

    fn notify_peer<
        'network,
        Conn: RdmaConnection + 'network,
        GroupConns: RdmaNetworkSelfGroupConnections<'network, Conn>,
    >(
        &mut self,
        idx: usize,
        connections: &mut GroupConns,
        timeout: Duration,
    ) -> Result<Duration, RdmaNetworkBarrierError> {
        let peer_conn = connections.connection_mut(idx);
        if let Some(RdmaNetworkSelfGroupConnection::PeerConnection(peer_rank_id, conn)) = peer_conn
        {
            let peer_mr_pair = self.connection_mr(peer_rank_id);
            let mut wr = conn
                .post_write(
                    peer_mr_pair.local_mr,
                    0..=0,
                    peer_mr_pair.remote_mr,
                    self.remote_peer_flag_mr_range(peer_rank_id),
                    None,
                )
                .map_err(|error| {
                    RdmaNetworkBarrierError::RdmaError(format!(
                        "Error issuing RDMA write to {peer_rank_id:?}: {error}"
                    ))
                })?;
            let (_, elapsed) = wr.spin_poll_batched(timeout, 1024).map_err(|error| {
                RdmaNetworkBarrierError::RdmaError(format!(
                    "Error during RDMA write to left child: {error}"
                ))
            })?;
            Ok(elapsed)
        } else {
            Ok(Duration::from_nanos(0))
        }
    }
}
