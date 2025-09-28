use crate::network::NetworkOp;
use crate::rdma_traits::{RdmaRendezvous, RdmaSendRecv};
use crate::synchronization::SyncError;

pub struct BinaryTreeSync;

impl NetworkOp for BinaryTreeSync {
    type Output = Result<(), SyncError>;

    fn run<'a, T: 'a + RdmaSendRecv + RdmaRendezvous>(
        &self,
        self_idx: Option<usize>,
        group_connections: &mut [&'a mut T],
    ) -> Self::Output {
        let self_idx = self_idx.ok_or(SyncError::SelfNotInSyncGroupError)?;

        if group_connections.is_empty() {
            return Err(SyncError::EmptyGroup);
        }

        // First all nodes rendezvous with their child and propagate the rendezvous up the tree
        Self::rendezvous_children(self_idx, group_connections)?;
        Self::rendezvous_parent(self_idx, group_connections)?;

        // After this, all nodes wait for a rendezvous from their parent and propagate downwards
        // This guarantees the opposite side of the tree is also synced
        Self::rendezvous_parent(self_idx, group_connections)?;
        Self::rendezvous_children(self_idx, group_connections)?;

        Ok(())
    }
}

impl BinaryTreeSync {
    fn rendezvous_children<'a, T: 'a + RdmaSendRecv + RdmaRendezvous>(
        self_idx: usize,
        group_connections: &mut [&'a mut T],
    ) -> Result<(), SyncError> {
        // First child rendezvous
        if let Some(conn) = group_connections.get_mut(self_idx * 2 + 1) {
            conn.rendezvous()?;
        }

        // Second child rendezvous
        if let Some(conn) = group_connections.get_mut(self_idx * 2 + 2) {
            conn.rendezvous()?;
        }

        Ok(())
    }

    fn rendezvous_parent<'a, T: 'a + RdmaSendRecv + RdmaRendezvous>(
        self_idx: usize,
        group_connections: &mut [&'a mut T],
    ) -> Result<(), SyncError> {
        // Only rendezvous with parent if not root node
        if self_idx != 0 {
            let conn = group_connections.get_mut((self_idx - 1) / 2).unwrap();
            conn.rendezvous()?;
        }

        Ok(())
    }
}
