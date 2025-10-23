use crate::barrier::{NonMatchingMemoryRegionCount, RdmaNetworkBarrier, RdmaNetworkBarrierError, RdmaNetworkMemoryRegionComponent};
use crate::rdma_connection::{
    RdmaConnection, RdmaWorkRequest, WorkRequestSpinPollError,
};
use crate::rdma_network_node::{
    RdmaNetworkSelfGroupConnection, RdmaNetworkSelfGroupConnections,
};
use crate::spin_poll::spin_poll_batched;
use std::ptr::{read_volatile, write_volatile};
use std::time::Duration;

pub struct RdmaNetworkUnregisteredCentralizedBarrier {
    memory: Vec<u8>,
}

#[derive(Debug)]
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

impl RdmaNetworkUnregisteredCentralizedBarrier {
    fn memory_of_connection(&mut self, rank_id: usize) -> (*mut u8, usize) {
        let ptr = &mut self.memory[rank_id * BYTES_PER_CONNECTION] as *mut u8;
        (ptr, BYTES_PER_CONNECTION)
    }
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
            .map(|conn_idx| self.memory_of_connection(conn_idx))
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

impl<MR, RemoteMR> RdmaNetworkBarrier<MR, RemoteMR>
    for RdmaNetworkCentralizedBarrier<MR, RemoteMR>
{
    type Error = RdmaNetworkBarrierError;

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

        if idx == 0 {
            // Coordinator
            self.coordinator_barrier(connections, timeout)
        } else {
            // Coordinated
            self.coordinated_barrier(connections, timeout)
        }
    }
}

impl<MR, RemoteMR> RdmaNetworkCentralizedBarrier<MR, RemoteMR> {
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
    ) -> Result<Duration, RdmaNetworkBarrierError> {
        let (_, elapsed) = spin_poll_batched(
            || (self.read_remote_peer_flag(rank_id) == READY_FLAG).then_some(()),
            timeout,
            1024,
        )
        .map_err(|_| {
            RdmaNetworkBarrierError::Timeout(format!(
                "Timeout waiting for peer {rank_id}"
            ))
        })?;
        self.reset_remote_peer_flag(rank_id);
        Ok(elapsed)
    }

    fn notify_peer<Conn: RdmaConnection<MR = MR, RemoteMR = RemoteMR>>(
        &self,
        rank_id: usize,
        conn: &mut Conn,
        timeout: Duration,
    ) -> Result<Duration, RdmaNetworkBarrierError> {
        let (mr, rmr) = &self.mrs[rank_id];
        let mut wr = conn.post_write(mr, 0..=0, rmr, 1..=1, None).map_err(|e| {
            RdmaNetworkBarrierError::RdmaError(format!("Write error: {e}"))
        })?;

        let (_, elapsed) = wr
            .spin_poll_batched(timeout, 1024)
            .map_err(|error| match error {
                WorkRequestSpinPollError::Timeout(_) => {
                    RdmaNetworkBarrierError::Timeout(format!(
                        "Timeout notifying peer {rank_id}"
                    ))
                }
                WorkRequestSpinPollError::ExecutionError(e) => {
                    RdmaNetworkBarrierError::RdmaError(format!("RDMA error: {e}"))
                }
            })?;
        Ok(elapsed)
    }

    fn coordinator_barrier<
        'network,
        Conn: RdmaConnection<MR = MR, RemoteMR = RemoteMR> + 'network,
        GroupConns: RdmaNetworkSelfGroupConnections<'network, Conn>,
    >(
        &mut self,
        mut connections: GroupConns,
        timeout: Duration,
    ) -> Result<(), RdmaNetworkBarrierError> {
        let mut available_time = timeout;

        // Wait for all peers
        for idx in 0..connections.len() {
            if let RdmaNetworkSelfGroupConnection::PeerConnection(rank_id, _) =
                connections.connection_mut(idx).unwrap()
            {
                available_time -= self.wait_for_peer(rank_id, available_time)?;
            }
        }

        // Notify all peers
        for idx in 0..connections.len() {
            if let RdmaNetworkSelfGroupConnection::PeerConnection(rank_id, conn) =
                connections.connection_mut(idx).unwrap()
            {
                available_time -= self.notify_peer(rank_id, conn, available_time)?;
            }
        }

        Ok(())
    }

    fn coordinated_barrier<
        'network,
        Conn: RdmaConnection<MR = MR, RemoteMR = RemoteMR> + 'network,
        GroupConns: RdmaNetworkSelfGroupConnections<'network, Conn>,
    >(
        &mut self,
        mut connections: GroupConns,
        timeout: Duration,
    ) -> Result<(), RdmaNetworkBarrierError> {
        let RdmaNetworkSelfGroupConnection::PeerConnection(rank_id, conn) =
            connections.connection_mut(0).unwrap()
        else {
            panic!("Coordinator must be at group index 0");
        };

        let elapsed = self.notify_peer(rank_id, conn, timeout)?;
        self.wait_for_peer(rank_id, timeout - elapsed)?;
        Ok(())
    }
}
