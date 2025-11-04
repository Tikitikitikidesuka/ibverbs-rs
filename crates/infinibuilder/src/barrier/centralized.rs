use crate::barrier::{
    NonMatchingMemoryRegionCount, RdmaNetworkNodeBarrier, RdmaNetworkNodeBarrierError,
};
use crate::rdma_connection::{
    RdmaConnection, RdmaMemoryRegionConnection, RdmaRemoteMemoryRegionConnection, RdmaWorkRequest,
    WorkRequestSpinPollError,
};
use crate::rdma_network_node::{
    MemoryRegionPair, RdmaNetworkMemoryRegionComponent, RdmaNetworkSelfGroupConnection,
    RdmaNetworkSelfGroupConnections,
};
use crate::spin_poll::spin_poll_timeout_batched;
use std::marker::PhantomData;
use std::ptr::{read_volatile, write_volatile};
use std::time::Duration;

#[derive(Debug)]
pub struct UnregisteredCentralizedBarrier<Connection: RdmaConnection> {
    memory: Vec<u8>,
    phantom_data:
        PhantomData<MemoryRegionPair<Connection::MemoryRegion, Connection::RemoteMemoryRegion>>,
}

#[derive(Debug)]
pub struct CentralizedBarrier<Connection: RdmaConnection> {
    memory: Vec<u8>,
    mrs: Vec<MemoryRegionPair<Connection::MemoryRegion, Connection::RemoteMemoryRegion>>,
}

impl<Connection: RdmaConnection> CentralizedBarrier<Connection> {
    pub fn new() -> UnregisteredCentralizedBarrier<Connection> {
        UnregisteredCentralizedBarrier {
            memory: vec![],
            phantom_data: Default::default(),
        }
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

impl<Connection: RdmaConnection> UnregisteredCentralizedBarrier<Connection> {
    fn memory_of_connection(&mut self, rank_id: usize) -> (*mut u8, usize) {
        let ptr = &mut self.memory[rank_id * BYTES_PER_CONNECTION] as *mut u8;
        (ptr, BYTES_PER_CONNECTION)
    }
}

impl<Connection: RdmaConnection>
    RdmaNetworkMemoryRegionComponent<Connection::MemoryRegion, Connection::RemoteMemoryRegion>
    for UnregisteredCentralizedBarrier<Connection>
{
    type Registered = CentralizedBarrier<Connection>;
    type RegisterError = NonMatchingMemoryRegionCount;

    fn memory(&mut self, num_connections: usize) -> Option<Vec<(*mut u8, usize)>> {
        self.memory = setup_memory(num_connections);
        Some(
            (0..num_connections)
                .into_iter()
                .map(|conn_idx| self.memory_of_connection(conn_idx))
                .collect(),
        )
    }

    fn registered_mrs(
        self,
        mrs: Option<
            Vec<MemoryRegionPair<Connection::MemoryRegion, Connection::RemoteMemoryRegion>>,
        >,
    ) -> Result<Self::Registered, Self::RegisterError> {
        let num_connections = self.memory.len() / BYTES_PER_CONNECTION;
        if let Some(mrs) = mrs {
            if mrs.len() != num_connections {
                return Err(NonMatchingMemoryRegionCount {
                    expected: num_connections,
                    got: mrs.len(),
                });
            }

            Ok(CentralizedBarrier {
                memory: self.memory,
                mrs,
            })
        } else {
            Err(NonMatchingMemoryRegionCount {
                expected: num_connections,
                got: 0,
            })
        }
    }
}

impl<Connection: RdmaConnection> RdmaNetworkNodeBarrier<Connection> for CentralizedBarrier<Connection> {
    type Error = RdmaNetworkNodeBarrierError;

    fn barrier<
        'network,
        GroupConns: RdmaNetworkSelfGroupConnections<'network, Connection = Connection>,
    >(
        &mut self,
        connections: GroupConns,
        timeout: Duration,
    ) -> Result<(), Self::Error> {
        let idx = connections.self_idx();

        if idx == 0 {
            // Coordinator
            println!("Running coordinator");
            self.coordinator_barrier(connections, timeout)
        } else {
            // Coordinated
            println!("Running coordinated");
            self.coordinated_barrier(connections, timeout)
        }
    }
}

impl<Connection: RdmaConnection> CentralizedBarrier<Connection> {
    fn read_remote_peer_flag(&self, rank_id: usize) -> u8 {
        let ptr = &self.memory[rank_id * BYTES_PER_CONNECTION + 1] as *const u8;
        unsafe { read_volatile(ptr) }
    }

    fn reset_remote_peer_flag(&mut self, rank_id: usize) {
        let ptr = &mut self.memory[rank_id * BYTES_PER_CONNECTION + 1] as *mut u8;
        unsafe { write_volatile(ptr, NOT_READY_FLAG) };
    }

    fn wait_for_peer(
        &mut self,
        rank_id: usize,
        timeout: Duration,
    ) -> Result<Duration, RdmaNetworkNodeBarrierError> {
        let (_, elapsed) = spin_poll_timeout_batched(
            || match self.read_remote_peer_flag(rank_id) == READY_FLAG {
                true => Ok(()),
                false => Err(()),
            },
            timeout,
            1024,
        )
        .map_err(|_| {
            RdmaNetworkNodeBarrierError::Timeout(format!("Timeout waiting for peer {rank_id}"))
        })?;
        self.reset_remote_peer_flag(rank_id);
        Ok(elapsed)
    }

    fn notify_peer(
        &self,
        rank_id: usize,
        conn: &mut Connection,
        timeout: Duration,
    ) -> Result<Duration, RdmaNetworkNodeBarrierError> {
        let peer_mr_pair = &self.mrs[rank_id];
        let mut wr = conn
            .post_write(
                &peer_mr_pair.local_mr,
                0..=0,
                &peer_mr_pair.remote_mr,
                1..=1,
                None,
            )
            .map_err(|e| RdmaNetworkNodeBarrierError::RdmaError(format!("Write error: {e}")))?;

        let (_, elapsed) = wr
            .spin_poll_batched(timeout, 1024)
            .map_err(|error| match error {
                WorkRequestSpinPollError::Timeout => RdmaNetworkNodeBarrierError::Timeout(format!(
                    "Timeout notifying peer {rank_id}"
                )),
                WorkRequestSpinPollError::ExecutionError(e) => {
                    RdmaNetworkNodeBarrierError::RdmaError(format!("RDMA error: {e}"))
                }
            })?;
        Ok(elapsed)
    }

    fn coordinator_barrier<
        'network,
        GroupConns: RdmaNetworkSelfGroupConnections<'network, Connection = Connection>,
    >(
        &mut self,
        mut connections: GroupConns,
        timeout: Duration,
    ) -> Result<(), RdmaNetworkNodeBarrierError> {
        let mut available_time = timeout;

        // Wait for all peers
        for idx in 0..connections.len() {
            if let RdmaNetworkSelfGroupConnection::PeerConnection(rank_id, _) =
                connections.connection_mut(idx).unwrap()
            {
                println!("Waiting for peer {idx}");
                available_time -= self.wait_for_peer(rank_id, available_time)?;
            }
        }

        // Notify all peers
        for idx in 0..connections.len() {
            if let RdmaNetworkSelfGroupConnection::PeerConnection(rank_id, conn) =
                connections.connection_mut(idx).unwrap()
            {
                println!("Notifying peer {idx}");
                available_time -= self.notify_peer(rank_id, conn, available_time)?;
            }
        }

        Ok(())
    }

    fn coordinated_barrier<
        'network,
        GroupConns: RdmaNetworkSelfGroupConnections<'network, Connection = Connection>,
    >(
        &mut self,
        mut connections: GroupConns,
        timeout: Duration,
    ) -> Result<(), RdmaNetworkNodeBarrierError> {
        let RdmaNetworkSelfGroupConnection::PeerConnection(rank_id, conn) =
            connections.connection_mut(0).unwrap()
        else {
            panic!("Coordinator must be at group index 0");
        };

        println!("Notifying peer");
        let elapsed = self.notify_peer(rank_id, conn, timeout)?;
        println!("Waiting for peer");
        self.wait_for_peer(rank_id, timeout - elapsed)?;
        Ok(())
    }
}
