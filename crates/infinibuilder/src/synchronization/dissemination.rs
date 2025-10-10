use crate::network::NetworkOp;
use crate::rdma_traits::{RdmaSendRecv, RdmaSyncSendRecv, RdmaSyncSignal};
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

    fn run<'a, T: 'a + RdmaSendRecv + RdmaSyncSignal>(
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
        while distance < max_distance {
            let self_left_idx = left_idx(self_idx, group_connections, distance);
            let self_right_idx = right_idx(self_idx, group_connections, distance);

            group_connections[self_right_idx].issue_signal();

            println!("Signaling right {distance} -> {self_right_idx}");
            group_connections[self_right_idx].signal_peer().unwrap()?;

            if self_right_idx != right_idx(self_right_idx, group_connections, distance) {
                println!("Waiting left {distance} -> {self_left_idx}");
                group_connections[self_left_idx].wait_for_new_barrier();
            }

            println!("Synchronizing left {distance} -> {self_left_idx}");
            group_connections[self_left_idx].synchronize()?;
            println!("Synchronizing right {distance} -> {self_right_idx}");
            group_connections[self_right_idx].synchronize()?;

            distance *= 2;
        }

        println!("Done!");

        Ok(())
    }
}

fn right_idx<T: RdmaSendRecv + RdmaSyncSignal>(
    self_idx: usize,
    group_connections: &mut [&mut T],
    distance: usize,
) -> usize {
    add_mod(self_idx, distance, group_connections.len())
}

fn left_idx<T: RdmaSendRecv + RdmaSyncSignal>(
    self_idx: usize,
    group_connections: &mut [&mut T],
    distance: usize,
) -> usize {
    sub_mod(self_idx, distance, group_connections.len())
}

fn add_mod(a: usize, b: usize, m: usize) -> usize {
    (a + b) % m
}

fn sub_mod(a: usize, b: usize, m: usize) -> usize {
    (a + m - b) % m
}
