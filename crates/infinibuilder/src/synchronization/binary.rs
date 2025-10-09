use std::time::Duration;
use crate::network::NetworkOp;
use crate::rdma_traits::{RdmaSync, RdmaSendRecv};
use crate::synchronization::SyncError;
use crate::synchronization::rendezvous_fn::{NoTimeoutSyncFn, SyncFn, TimeoutSyncFn};

#[derive(Debug, Copy, Clone)]
pub struct BinaryTreeSync<D: SyncFn> {
    rendezvous_fn: D,
}

impl BinaryTreeSync<NoTimeoutSyncFn> {
    pub fn new() -> BinaryTreeSync<NoTimeoutSyncFn> {
        BinaryTreeSync {
            rendezvous_fn: NoTimeoutSyncFn,
        }
    }

    pub fn with_timeout(timeout: Duration) -> BinaryTreeSync<TimeoutSyncFn> {
        BinaryTreeSync {
            rendezvous_fn: TimeoutSyncFn { timeout },
        }
    }
}

impl<D: SyncFn> NetworkOp for BinaryTreeSync<D> {
    type Output = Result<(), SyncError>;

    fn run<'a, T: 'a + RdmaSendRecv + RdmaSync>(
        &self,
        self_idx: Option<usize>,
        group_connections: &mut [&'a mut T],
    ) -> Self::Output {
        let self_idx = self_idx.ok_or(SyncError::SelfNotInSyncGroupError)?;

        if group_connections.is_empty() {
            return Err(SyncError::EmptyGroup);
        }

        // Wait until children, if they exist, notify us
        left_child_connection(self_idx, group_connections)
            .map(|conn| self.rendezvous_fn.wait_for_peer_signal(conn))
            .transpose()?;
        right_child_connection(self_idx, group_connections)
            .map(|conn| self.rendezvous_fn.wait_for_peer_signal(conn))
            .transpose()?;

        // Rendezvous with parent, if it exists
        parent_connection(self_idx, group_connections)
            .map(|conn| self.rendezvous_fn.rendezvous(conn))
            .transpose()?;

        // Rendezvous with children, if they exist, to notify them back
        left_child_connection(self_idx, group_connections)
            .map(|conn| self.rendezvous_fn.rendezvous(conn))
            .transpose()?;
        right_child_connection(self_idx, group_connections)
            .map(|conn| self.rendezvous_fn.rendezvous(conn))
            .transpose()?;

        Ok(())
    }
}

fn left_child_connection<'a, T: RdmaSendRecv + RdmaSync>(
    self_idx: usize,
    group_connections: &'a mut [&mut T],
) -> Option<&'a mut T> {
    group_connections
        .get_mut(self_idx * 2 + 1)
        .map(|c| &mut **c)
}

fn right_child_connection<'a, T: RdmaSendRecv + RdmaSync>(
    self_idx: usize,
    group_connections: &'a mut [&mut T],
) -> Option<&'a mut T> {
    group_connections
        .get_mut(self_idx * 2 + 2)
        .map(|c| &mut **c)
}

fn parent_connection<'a, T: RdmaSendRecv + RdmaSync>(
    self_idx: usize,
    group_connections: &'a mut [&mut T],
) -> Option<&'a mut T> {
    // No parent if root node
    if self_idx != 0 {
        Some(group_connections[(self_idx - 1) / 2])
    } else {
        None
    }
}
