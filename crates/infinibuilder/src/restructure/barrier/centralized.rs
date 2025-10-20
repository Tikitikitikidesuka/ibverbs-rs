use crate::restructure::barrier::{RdmaNetworkBarrier, RdmaNetworkMemoryRegionComponent};
use crate::restructure::rdma_connection::RdmaConnection;
use crate::restructure::rdma_network_node::{
    RdmaNetworkSelfGroupConnection, RdmaNetworkSelfGroupConnections,
};
use std::ptr::read_volatile;
use std::time::Duration;
use thiserror::Error;

pub struct RdmaNetworkUnregisteredCentralizedBarrier {
    memory: Vec<u8>,
}

pub struct RdmaNetworkCentralizedBarrier<MR, RMR> {
    memory: Vec<u8>,
    mrs: Vec<(MR, RMR)>,
}

impl RdmaNetworkCentralizedBarrier<(), ()> {
    pub fn new() -> RdmaNetworkUnregisteredCentralizedBarrier {
        RdmaNetworkUnregisteredCentralizedBarrier { memory: vec![] }
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

fn memory_of_connection(memory: &mut [u8], conn_idx: usize) -> (*mut u8, usize) {
    let ptr = &mut memory[conn_idx * BYTES_PER_CONNECTION] as *mut u8;
    (ptr, BYTES_PER_CONNECTION)
}

fn read_remote_peer_flag(memory: &[u8], conn_idx: usize) -> u8 {
    let ptr = &memory[conn_idx * BYTES_PER_CONNECTION + 1] as *const u8;
    unsafe { read_volatile(ptr) }
}

#[derive(Debug, Error)]
#[error("Non matching memory region count, expected {expected}, got {got}")]
pub struct NonMatchingMemoryRegionCount {
    expected: usize,
    got: usize,
}

impl<MR, RMR> RdmaNetworkMemoryRegionComponent<MR, RMR>
    for RdmaNetworkUnregisteredCentralizedBarrier
{
    type Registered = RdmaNetworkCentralizedBarrier<MR, RMR>;
    type RegisterError = NonMatchingMemoryRegionCount;

    fn memory(&mut self, num_connections: usize) -> Vec<(*mut u8, usize)> {
        self.memory = setup_memory(num_connections);
        (0..num_connections)
            .into_iter()
            .map(|conn_idx| memory_of_connection(self.memory.as_mut_slice(), conn_idx))
            .collect()
    }

    fn registered_mrs(self, mrs: Vec<(MR, RMR)>) -> Result<Self::Registered, Self::RegisterError> {
        let num_connections = self.memory.len() / BYTES_PER_CONNECTION;
        if mrs.len() != num_connections {
            return Err(NonMatchingMemoryRegionCount {
                expected: num_connections,
                got: mrs.len(),
            });
        }

        Ok(RdmaNetworkCentralizedBarrier {
            memory: self.memory,
            mrs,
        })
    }
}

impl<MR, RMR> RdmaNetworkBarrier for RdmaNetworkCentralizedBarrier<MR, RMR> {
    type Error = ();

    fn barrier<
        'network,
        Conn: RdmaConnection + 'network,
        GroupConns: RdmaNetworkSelfGroupConnections<'network, Conn>,
    >(
        &mut self,
        mut connections: GroupConns,
        timeout: Duration,
    ) -> Result<(), Self::Error> {
        let idx = connections.self_idx();

        if idx == 0 {
            // Coordinator
            self.coordinator_barrier(connections, timeout)
        } else {
            // Coordinated
            // Groups can never be empty
            if let RdmaNetworkSelfGroupConnection::PeerConnection(rank_id, master_conn) =
                connections.connection_mut(0).unwrap()
            {
                self.coordinated_barrier(master_conn, timeout)
            } else {
                panic!(
                    "Coordinator of centralized sync must always be group index zero. \
                     Check group construction is being handled properly"
                );
            }
        }
    }
}

impl<MR, RMR> RdmaNetworkCentralizedBarrier<MR, RMR> {
    fn coordinator_barrier<
        'network,
        Conn: RdmaConnection + 'network,
        GroupConns: RdmaNetworkSelfGroupConnections<'network, Conn>,
    >(
        &self,
        mut connections: GroupConns,
        timeout: Duration,
    ) -> Result<(), ()> {
        for idx in 0..connections.len() {
            if let RdmaNetworkSelfGroupConnection::PeerConnection(rank_id, peer_conn) =
                connections.connection_mut(idx).unwrap()
            {
                println!("Connecting to peer {rank_id} as central barrier coordinator");
            }
        }
        todo!()
    }

    fn coordinated_barrier<Conn: RdmaConnection>(
        &self,
        master_conn: &mut Conn,
        timeout: Duration,
    ) -> Result<(), ()> {
        println!("Connecting to coordinator");
        todo!()
    }
}
