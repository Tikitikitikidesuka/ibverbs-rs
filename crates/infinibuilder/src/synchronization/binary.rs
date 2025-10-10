use crate::network::NetworkOp;
use crate::rdma_traits::{RdmaSendRecv, RdmaSync};
use crate::synchronization::SyncError;

#[derive(Debug, Copy, Clone)]
pub struct BinaryTreeSync;

impl BinaryTreeSync {
    pub fn new() -> Self {
        Self
    }
}

impl NetworkOp for BinaryTreeSync {
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
        left_child_connection(self_idx, group_connections).map(|conn| conn.wait_for_new_barrier());
        right_child_connection(self_idx, group_connections).map(|conn| conn.wait_for_new_barrier());

        // Rendezvous with parent, if it exists
        parent_connection(self_idx, group_connections)
            .map(|conn| {
                conn.signal_peer()
                    .ok_or(std::io::Error::new(std::io::ErrorKind::Other, "Desync"))??;
                conn.synchronize()
            })
            .transpose()?;

        // Rendezvous with children, if they exist, to notify them back
        left_child_connection(self_idx, group_connections)
            .map(|conn| conn.synchronize())
            .transpose()?;
        right_child_connection(self_idx, group_connections)
            .map(|conn| conn.synchronize())
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
