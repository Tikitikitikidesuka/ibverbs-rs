use crate::network::NetworkOp;
use crate::rdma_traits::{RdmaRendezvous, RdmaSendRecv};
use crate::synchronization::SyncError;

pub struct CentralizedSync;

impl NetworkOp for CentralizedSync {
    type Output = Result<(), SyncError>;

    fn run<'a, T: 'a + RdmaSendRecv + RdmaRendezvous>(
        &self,
        self_idx: Option<usize>,
        group_connections: &mut [&'a mut T],
    ) -> Self::Output {
        let self_idx = self_idx.ok_or(SyncError::SelfNotInSyncGroupError)?;

        let master_conn = group_connections.get_mut(0).ok_or(SyncError::EmptyGroup)?;

        if self_idx == 0 {
            // If first node of the group, master role
            group_connections
                .into_iter()
                .try_for_each(|conn| conn.rendezvous())?;
        } else {
            // If first node of the group, slave role
            master_conn.rendezvous()?;
        }

        Ok(())
    }
}
