// Communicates when a receive has been issued and waits for its signal

use crate::rdma_connection::{
    RdmaConnection, RdmaMemoryRegionConnection, RdmaPostReadConnection, RdmaPostReceiveConnection,
    RdmaPostReceiveImmediateDataConnection, RdmaPostSendConnection,
    RdmaPostSendImmediateDataConnection, RdmaPostWriteConnection, RdmaRemoteMemoryRegionConnection,
    RdmaWorkRequest,
};
use crate::rdma_network_node::{
    MemoryRegionPair, NonMatchingMemoryRegionCount, RdmaNetworkMemoryRegionComponent,
};
use crate::spin_poll::spin_poll_timeout_batched;
use crate::transport::{
    RdmaNetworkNodeReadTransport, RdmaNetworkNodeReceiveImmediateDataTransport,
    RdmaNetworkNodeReceiveTransport, RdmaNetworkNodeSendImmediateDataTransport,
    RdmaNetworkNodeSendTransport, RdmaNetworkNodeWriteTransport,
};
use derivative::Derivative;
use std::error::Error;
use std::marker::PhantomData;
use std::ops::RangeBounds;
use std::ptr::{read, read_volatile, write, write_volatile};
use std::time::Duration;
use thiserror::Error;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct UnregisteredSyncedTransport<Connection: RdmaConnection> {
    post_timeout: Duration,
    memory: Vec<u8>,
    #[derivative(Debug = "ignore")]
    phantom: PhantomData<Connection>,
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct SyncedTransport<Connection: RdmaConnection> {
    post_timeout: Duration,
    memory: Vec<u8>,
    #[derivative(Debug = "ignore")]
    mrs: Vec<MemoryRegionPair<Connection::MemoryRegion, Connection::RemoteMemoryRegion>>,
}

impl<Connection: RdmaConnection> SyncedTransport<Connection> {
    pub fn with_post_timeout(timeout: Duration) -> UnregisteredSyncedTransport<Connection> {
        UnregisteredSyncedTransport {
            post_timeout: timeout,
            memory: vec![],
            phantom: Default::default(),
        }
    }
}

/// Three u64 per connection, first is local counter of issued receives.
/// The second is the counter of remote issued receives.
/// The third is a counter of local issued sends.
/// When a connection issues a receive, it adds one to its counter of issued receives.
/// And RDMA writes it to the peers counter of remote issued receives.
/// A connection is only able to send when the counter remote issued receives
/// is higher than the counter of local issued sends.
/// When it sends, it adds one to its local counter of issued sends.

const BYTES_PER_CONNECTION: usize = 3 * size_of::<u64>();

fn setup_memory(num_connections: usize) -> Vec<u8> {
    // Assumes all machines in network have same endianness...
    // All counters initialized to zero
    vec![0u8; num_connections * BYTES_PER_CONNECTION]
}

impl<Connection: RdmaConnection> UnregisteredSyncedTransport<Connection> {
    fn memory_of_connection(&mut self, rank_id: usize) -> (*mut u8, usize) {
        let ptr = &mut self.memory[rank_id * BYTES_PER_CONNECTION] as *mut u8;
        (ptr, BYTES_PER_CONNECTION)
    }
}

impl<Connection: RdmaConnection> SyncedTransport<Connection> {
    fn local_issued_receives_ptr(&mut self, rank_id: usize) -> *mut u64 {
        &mut self.memory[rank_id * BYTES_PER_CONNECTION] as *mut u8 as *mut u64
    }

    fn remote_issued_receives_ptr(&mut self, rank_id: usize) -> *mut u64 {
        unsafe { (&mut self.memory[rank_id * BYTES_PER_CONNECTION] as *mut u8 as *mut u64).add(1) }
    }

    fn local_issued_sends_ptr(&mut self, rank_id: usize) -> *mut u64 {
        unsafe { (&mut self.memory[rank_id * BYTES_PER_CONNECTION] as *mut u8 as *mut u64).add(2) }
    }

    fn local_issued_receives_mr_range(&self) -> impl RangeBounds<usize> {
        (0 * size_of::<u64>())..(1 * size_of::<u64>())
    }

    fn remote_issued_receives_mr_range(&self) -> impl RangeBounds<usize> {
        (1 * size_of::<u64>())..(2 * size_of::<u64>())
    }

    fn read_local_issued_receives(&mut self, rank_id: usize) -> u64 {
        // Non-volatile read since it is only written into locally
        unsafe { read(self.local_issued_receives_ptr(rank_id)) }
    }

    fn increase_local_issued_receives(&mut self, rank_id: usize) {
        // Volatile write since it will be sent through RDMA write
        unsafe {
            write_volatile(
                self.local_issued_receives_ptr(rank_id),
                self.read_local_issued_receives(rank_id) + 1,
            )
        };
    }

    fn read_remote_issued_receives(&mut self, rank_id: usize) -> u64 {
        // Volatile read since it written into via RDMA
        unsafe { read_volatile(self.remote_issued_receives_ptr(rank_id)) }
    }

    fn read_local_issued_sends(&mut self, rank_id: usize) -> u64 {
        // Non-volatile read since it's only used locally
        unsafe { read(self.local_issued_sends_ptr(rank_id)) }
    }

    fn increase_local_issued_sends(&mut self, rank_id: usize) {
        // Non-volatile write since it's only used locally
        unsafe {
            write(
                self.local_issued_sends_ptr(rank_id),
                self.read_local_issued_sends(rank_id) + 1,
            )
        };
    }
}

impl<Connection: RdmaConnection>
    RdmaNetworkMemoryRegionComponent<Connection::MemoryRegion, Connection::RemoteMemoryRegion>
    for UnregisteredSyncedTransport<Connection>
{
    type Registered = SyncedTransport<Connection>;
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

            Ok(SyncedTransport {
                post_timeout: self.post_timeout,
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

#[derive(Debug, Error)]
pub enum SyncedTransportError<E: Error> {
    #[error("Sync error: {0}")]
    SyncError(String),
    #[error("Sync timeout")]
    SyncTimeout,
    #[error("Operation error: {0}")]
    OperationError(E),
}

impl<Connection: RdmaConnection> RdmaNetworkNodeSendTransport<Connection>
    for SyncedTransport<Connection>
{
    type WorkRequest = <Connection as RdmaPostSendConnection>::WorkRequest;
    type PostError = SyncedTransportError<<Connection as RdmaPostSendConnection>::PostError>;

    fn post_send(
        &mut self,
        rank_id: usize,
        conn: &mut Connection,
        memory_region: &Connection::MemoryRegion,
        memory_range: impl RangeBounds<usize> + Clone,
        immediate_data: Option<u32>,
    ) -> Result<Self::WorkRequest, Self::PostError> {
        let local_issued_sends = self.read_local_issued_sends(rank_id);
        let timeout = self.post_timeout;

        //println!("Local issued sends: {local_issued_sends}");

        match spin_poll_timeout_batched(
            || {
                // If not all send tokens consumed, send
                let remote_issued_receives = self.read_remote_issued_receives(rank_id);
                //println!("Remote issued receives: {remote_issued_receives}");
                if local_issued_sends < self.read_remote_issued_receives(rank_id) {
                    self.increase_local_issued_sends(rank_id);
                    return conn
                        .post_send(memory_region, memory_range.clone(), immediate_data)
                        .map_err(|e| Some(SyncedTransportError::OperationError(e)));
                } else {
                    Err(None)
                }
            },
            timeout,
            1024,
        ) {
            Ok((wr, elapsed)) => Ok(wr),
            Err(None) => Err(SyncedTransportError::SyncTimeout),
            Err(Some(error)) => Err(error),
        }
    }
}

impl<Connection: RdmaConnection> RdmaNetworkNodeReceiveTransport<Connection>
    for SyncedTransport<Connection>
{
    type WorkRequest = <Connection as RdmaPostReceiveConnection>::WorkRequest;
    type PostError = SyncedTransportError<<Connection as RdmaPostReceiveConnection>::PostError>;

    fn post_receive(
        &mut self,
        rank_id: usize,
        conn: &mut Connection,
        memory_region: &Connection::MemoryRegion,
        memory_range: impl RangeBounds<usize>,
    ) -> Result<Self::WorkRequest, Self::PostError> {
        // First issue the receive
        let wr = conn
            .post_receive(memory_region, memory_range)
            .map_err(SyncedTransportError::OperationError)?;
        //println!("Issued receive");

        // Then notify the peer of it
        self.increase_local_issued_receives(rank_id);
        //println!("Local issued receives: {}", self.read_local_issued_receives(rank_id));
        conn.post_write(
            &self.mrs[rank_id].local_mr,
            self.local_issued_receives_mr_range(),
            &self.mrs[rank_id].remote_mr,
            self.remote_issued_receives_mr_range(),
            None,
        )
        .map_err(|e| {
            SyncedTransportError::SyncError(
                "Error writing local issued receives counter to remote peer. Could not post write operation.".to_string(),
            )
        })?
        .spin_poll_batched(self.post_timeout, 1024).map_err(|e| {
            SyncedTransportError::SyncError(
                format!("Error writing local issued receives counter to remote peer. Write operation failed: {e}")
            )
        })?;
        //println!("Notified the peer of receive");

        Ok(wr)
    }
}

impl<Connection: RdmaConnection> RdmaNetworkNodeWriteTransport<Connection>
    for SyncedTransport<Connection>
{
    type WorkRequest = <Connection as RdmaPostWriteConnection>::WorkRequest;
    type PostError = <Connection as RdmaPostWriteConnection>::PostError;

    fn post_write(
        &mut self,
        rank_id: usize,
        conn: &mut Connection,
        local_memory_region: &Connection::MemoryRegion,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: &Connection::RemoteMemoryRegion,
        remote_memory_range: impl RangeBounds<usize>,
        immediate_data: Option<u32>,
    ) -> Result<Self::WorkRequest, Self::PostError> {
        conn.post_write(
            local_memory_region,
            local_memory_range,
            remote_memory_region,
            remote_memory_range,
            immediate_data,
        )
    }
}

impl<Connection: RdmaConnection> RdmaNetworkNodeReadTransport<Connection>
    for SyncedTransport<Connection>
{
    type WorkRequest = <Connection as RdmaPostReadConnection>::WorkRequest;
    type PostError = <Connection as RdmaPostReadConnection>::PostError;

    fn post_read(
        &mut self,
        rank_id: usize,
        conn: &mut Connection,
        local_memory_region: &Connection::MemoryRegion,
        local_memory_range: impl RangeBounds<usize>,
        remote_memory_region: &Connection::RemoteMemoryRegion,
        remote_memory_range: impl RangeBounds<usize>,
    ) -> Result<Self::WorkRequest, Self::PostError> {
        conn.post_read(
            local_memory_region,
            local_memory_range,
            remote_memory_region,
            remote_memory_range,
        )
    }
}

impl<Connection: RdmaConnection> RdmaNetworkNodeSendImmediateDataTransport<Connection>
    for SyncedTransport<Connection>
{
    type WorkRequest = <Connection as RdmaPostSendImmediateDataConnection>::WorkRequest;
    type PostError =
        SyncedTransportError<<Connection as RdmaPostSendImmediateDataConnection>::PostError>;

    fn post_send_immediate_data(
        &mut self,
        rank_id: usize,
        conn: &mut Connection,
        immediate_data: u32,
    ) -> Result<Self::WorkRequest, Self::PostError> {
        let local_issued_sends = self.read_local_issued_sends(rank_id);
        let timeout = self.post_timeout;

        match spin_poll_timeout_batched(
            || {
                // If not all send tokens consumed, send
                if local_issued_sends < self.read_remote_issued_receives(rank_id) {
                    return conn
                        .post_send_immediate_data(immediate_data)
                        .map_err(|e| Some(SyncedTransportError::OperationError(e)));
                } else {
                    Err(None)
                }
            },
            timeout,
            1024,
        ) {
            Ok((wr, elapsed)) => Ok(wr),
            Err(None) => Err(SyncedTransportError::SyncTimeout),
            Err(Some(error)) => Err(error),
        }
    }
}

impl<Connection: RdmaConnection> RdmaNetworkNodeReceiveImmediateDataTransport<Connection>
    for SyncedTransport<Connection>
{
    type WorkRequest = <Connection as RdmaPostReceiveImmediateDataConnection>::WorkRequest;
    type PostError =
        SyncedTransportError<<Connection as RdmaPostReceiveImmediateDataConnection>::PostError>;

    fn post_receive_immediate_data(
        &mut self,
        rank_id: usize,
        conn: &mut Connection,
    ) -> Result<Self::WorkRequest, Self::PostError> {
        // First issue the receive
        let wr = conn
            .post_receive_immediate_data()
            .map_err(SyncedTransportError::OperationError)?;

        // Then notify the peer of it
        self.increase_local_issued_receives(rank_id);
        conn.post_write(
            &self.mrs[rank_id].local_mr,
            self.local_issued_receives_mr_range(),
            &self.mrs[rank_id].remote_mr,
            self.remote_issued_receives_mr_range(),
            None,
        )
            .map_err(|e| {
                SyncedTransportError::SyncError(
                    "Error writing local issued receives counter to remote peer. Could not post write operation.".to_string(),
                )
            })?
            .spin_poll_batched(self.post_timeout, 1024).map_err(|e| {
            SyncedTransportError::SyncError(
                format!("Error writing local issued receives counter to remote peer. Write operation failed: {e}")
            )
        })?;

        Ok(wr)
    }
}
