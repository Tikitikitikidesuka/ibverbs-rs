use crate::network::NetworkOp;
use crate::rdma_traits::{RdmaRendezvous, RdmaSendRecv};
use crate::synchronization::SyncError;
use crate::synchronization::rendezvous_fn::{
    NoTimeoutRendezvousFn, RendezvousFn, TimeoutRendezvousFn,
};
use std::time::Duration;

#[derive(Debug, Copy, Clone)]
pub struct DisseminationSync<D: RendezvousFn> {
    rendezvous_fn: D,
}

impl DisseminationSync<NoTimeoutRendezvousFn> {
    pub fn new() -> DisseminationSync<NoTimeoutRendezvousFn> {
        DisseminationSync {
            rendezvous_fn: NoTimeoutRendezvousFn,
        }
    }

    pub fn with_timeout(timeout: Duration) -> DisseminationSync<TimeoutRendezvousFn> {
        DisseminationSync {
            rendezvous_fn: TimeoutRendezvousFn { timeout },
        }
    }
}

impl<D: RendezvousFn> NetworkOp for DisseminationSync<D> {
    type Output = Result<(), SyncError>;

    fn run<'a, T: 'a + RdmaSendRecv + RdmaRendezvous>(
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
            if (self_idx / distance) % 2 == 0 {
                self.rendezvous_fn
                    .rendezvous(right_connection(self_idx, group_connections, distance))?;
                self.rendezvous_fn
                    .rendezvous(left_connection(self_idx, group_connections, distance))?;
            } else {
                self.rendezvous_fn
                    .rendezvous(left_connection(self_idx, group_connections, distance))?;
                self.rendezvous_fn
                    .rendezvous(right_connection(self_idx, group_connections, distance))?;
            }

            distance *= 2;
        }

        Ok(())
    }
}

fn right_connection<'a, T: RdmaSendRecv + RdmaRendezvous>(
    self_idx: usize,
    group_connections: &'a mut [&mut T],
    distance: usize,
) -> &'a mut T {
    let idx = add_mod(self_idx, distance, group_connections.len());
    &mut group_connections[idx]
}

fn left_connection<'a, T: RdmaSendRecv + RdmaRendezvous>(
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
