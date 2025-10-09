use crate::network::NetworkOp;
use crate::rdma_traits::{RdmaSendRecv, RdmaSync};
use crate::synchronization::SyncError;

#[derive(Debug, Copy, Clone)]
pub struct DisseminationSync;

impl DisseminationSync {
    pub fn new() -> DisseminationSync {
        DisseminationSync {}
    }
}

impl NetworkOp for DisseminationSync {
    type Output = Result<(), SyncError>;

    fn run<'a, T: 'a + RdmaSendRecv + RdmaSync>(
        &self,
        self_idx: Option<usize>,
        group_connections: &mut [&mut T],
    ) -> Self::Output {
        let self_idx = self_idx.ok_or(SyncError::SelfNotInSyncGroupError)?;

        if group_connections.is_empty() {
            return Err(SyncError::EmptyGroup);
        }

        // TODO: EXPLAIN
        let mut distance = 1;
        let max_distance = group_connections.len();
        while distance <= max_distance {
            right_connection(self_idx, group_connections, distance).signal_peer().unwrap()?;
            left_connection(self_idx, group_connections, distance).synchronize()?;

            distance *= 2;
        }

        Ok(())
    }
}

fn right_connection<'a, T: RdmaSendRecv + RdmaSync>(
    self_idx: usize,
    group_connections: &'a mut [&mut T],
    distance: usize,
) -> &'a mut T {
    let idx = add_mod(self_idx, distance, group_connections.len());
    &mut group_connections[idx]
}

fn left_connection<'a, T: RdmaSendRecv + RdmaSync>(
    self_idx: usize,
    group_connections: &'a mut [&mut T],
    distance: usize,
) -> &'a mut T {
    let idx = sub_mod(self_idx, distance, group_connections.len());
    &mut group_connections[idx]
}

fn add_mod(a: usize, b: usize, m: usize) -> usize {
    (a + b) % m
}

fn sub_mod(a: usize, b: usize, m: usize) -> usize {
    (a + m - b) % m
}
