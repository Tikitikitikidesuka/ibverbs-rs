// Communicates when a receive has been issued and waits for its signal

use std::marker::PhantomData;
use crate::barrier::{MemoryRegionPair, NonMatchingMemoryRegionCount, RdmaNetworkBarrier, RdmaNetworkMemoryRegionComponent};

#[derive(Debug)]
pub struct UnregisteredSyncedTransfer<MR, RMR> {
    memory: Vec<u8>,
    phantom_data: PhantomData<(MR, RMR)>,
}

#[derive(Debug)]
pub struct SyncedTransfer<MR, RMR> {
    memory: Vec<u8>,
    mrs: Vec<MemoryRegionPair<MR, RMR>>,
}

impl<MR, RMR> SyncedTransfer<MR, RMR> {
    pub fn new() -> UnregisteredSyncedTransfer<MR, RMR> {
        UnregisteredSyncedTransfer { memory: vec![], phantom_data: Default::default() }
    }
}

/// Three u64 per connection, first is local counter of issued receives.
/// The second is the counter of send tokens.
/// The third is a counter of issued sends.
/// When a connection issues a receive, it adds one to its counter of issued receives.
/// And RDMA writes it to the peers counter of send tokens.
/// A connection is only able to send when the counter of available tokens
/// is higher than the counter of issued sends.
/// When it sends, it adds one to its counter of issued sends.

fn setup_memory(num_connections: usize) -> Vec<u8> {
    todo!()
}

impl<MR, RMR> UnregisteredSyncedTransfer<MR, RMR> {
    fn memory_of_connection(&mut self, rank_id: usize) -> (*mut u8, usize) {
        todo!()
    }
}

impl<MR, RMR> RdmaNetworkMemoryRegionComponent<MR, RMR> for UnregisteredSyncedTransfer<MR, RMR> {
    type Registered = SyncedTransfer<MR, RMR>;
    type RegisterError = NonMatchingMemoryRegionCount;

    fn memory(&mut self, num_connections: usize) -> Option<Vec<(*mut u8, usize)>> {
        todo!()
        /*
        self.memory = setup_memory(num_connections);
        (0..num_connections)
            .into_iter()
            .map(|conn_idx| self.memory_of_connection(conn_idx))
            .collect()
        */
    }

    fn registered_mrs(self, mrs: Option<Vec<MemoryRegionPair<MR, RMR>>>) -> Result<Self::Registered, Self::RegisterError> {
        todo!()
        /*
        let num_connections = self.memory.len() / BYTES_PER_CONNECTION;
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
        */
    }
}