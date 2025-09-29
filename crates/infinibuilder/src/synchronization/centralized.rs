use crate::network::NetworkOp;
use crate::rdma_traits::{RdmaRendezvous, RdmaSendRecv};
use crate::synchronization::SyncError;
use crate::synchronization::rendezvous_fn::{
    NoTimeoutRendezvousFn, RendezvousFn, TimeoutRendezvousFn,
};
use std::time::Duration;

#[derive(Debug, Copy, Clone)]
pub struct CentralizedSync<D: RendezvousFn> {
    rendezvous_fn: D,
}

impl CentralizedSync<NoTimeoutRendezvousFn> {
    pub fn new() -> CentralizedSync<NoTimeoutRendezvousFn> {
        CentralizedSync {
            rendezvous_fn: NoTimeoutRendezvousFn,
        }
    }

    pub fn with_timeout(timeout: Duration) -> CentralizedSync<TimeoutRendezvousFn> {
        CentralizedSync {
            rendezvous_fn: TimeoutRendezvousFn { timeout },
        }
    }
}

impl<D: RendezvousFn> NetworkOp for CentralizedSync<D> {
    type Output = Result<(), SyncError>;

    fn run<'a, T: 'a + RdmaSendRecv + RdmaRendezvous>(
        &self,
        self_idx: Option<usize>,
        group_connections: &mut [&'a mut T],
    ) -> Self::Output {
        let self_idx = self_idx.ok_or(SyncError::SelfNotInSyncGroupError)?;
        let master_conn = master_connection(group_connections).ok_or(SyncError::EmptyGroup)?;

        if self_idx == 0 {
            // Master role
            // First wait for each node
            slave_connections(group_connections)
                .iter_mut()
                .try_for_each(|conn| self.rendezvous_fn.wait_for_peer_signal(*conn))?;
            // Then rendezvous with all slaves
            slave_connections(group_connections)
                .iter_mut()
                .try_for_each(|conn| self.rendezvous_fn.rendezvous(*conn))?;
        } else {
            // Slave role
            // Rendezvous with master
            self.rendezvous_fn.rendezvous(master_conn)?;
        }

        Ok(())
    }
}

fn master_connection<'a, T: RdmaSendRecv + RdmaRendezvous>(
    group_connections: &'a mut [&mut T],
) -> Option<&'a mut T> {
    group_connections.get_mut(0).map(|c| &mut **c)
}

fn slave_connections<'a, 'b, T: RdmaSendRecv + RdmaRendezvous>(
    group_connections: &'a mut [&'b mut T],
) -> &'a mut [&'b mut T] {
    group_connections.split_at_mut(1).1
}
