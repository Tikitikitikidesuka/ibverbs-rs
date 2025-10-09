use crate::network::NetworkOp;
use crate::rdma_traits::{RdmaSendRecv, RdmaSync, SyncState, spin_poll};
use crate::synchronization::SyncError;

#[derive(Debug, Copy, Clone)]
pub struct CentralizedSync;

impl CentralizedSync {
    pub fn new() -> CentralizedSync {
        CentralizedSync {}
    }
}

impl NetworkOp for CentralizedSync {
    type Output = Result<(), SyncError>;

    fn run<'a, T: 'a + RdmaSendRecv + RdmaSync>(
        &self,
        self_idx: Option<usize>,
        group_connections: &mut [&'a mut T],
    ) -> Self::Output {
        let self_idx = self_idx.ok_or(SyncError::SelfNotInSyncGroupError)?;
        let master_conn = master_connection(group_connections).ok_or(SyncError::EmptyGroup)?;

        if self_idx == 0 {
            // Master role
            // First wait for each slave
            slave_connections(group_connections)
                .iter_mut()
                .for_each(|conn| spin_poll(|| conn.sync_state() == SyncState::Behind));

            // Then signal all slaves
            slave_connections(group_connections)
                .iter_mut()
                .try_for_each(|conn| conn.signal_peer().unwrap())?;
        } else {
            // Slave role
            // Synchronize new barrier with master
            master_conn.signal_peer();
            master_conn.synchronize()?;
        }

        Ok(())
    }
}

// TODO: TIMEOUT VERSION

fn master_connection<'a, T: RdmaSendRecv + RdmaSync>(
    group_connections: &'a mut [&mut T],
) -> Option<&'a mut T> {
    group_connections.get_mut(0).map(|c| &mut **c)
}

fn slave_connections<'a, 'b, T: RdmaSendRecv + RdmaSync>(
    group_connections: &'a mut [&'b mut T],
) -> &'a mut [&'b mut T] {
    group_connections.split_at_mut(1).1
}
