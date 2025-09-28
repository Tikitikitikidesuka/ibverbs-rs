use crate::network::NetworkOp;
use crate::rdma_traits::{RdmaRendezvous, RdmaSendRecv};
use crate::synchronization::SyncError;
use std::time::Duration;

#[derive(Debug, Copy, Clone)]
pub struct DisseminationSync;
// TODO: ADD TIMEOUT

impl NetworkOp for DisseminationSync {
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

        // TODO: EXPLAIN
        let mut distance = 1;
        while distance < group_connections.len() {
            if (self_idx / distance) % 2 == 0 {
                Self::rendezvous_right(self_idx, group_connections, distance)?;
                Self::rendezvous_left(self_idx, group_connections, distance)?;
            } else {
                Self::rendezvous_left(self_idx, group_connections, distance)?;
                Self::rendezvous_right(self_idx, group_connections, distance)?;
            }

            distance *= 2;
        }

        Ok(())
    }
}

impl DisseminationSync {
    fn rendezvous_right<'a, T: 'a + RdmaSendRecv + RdmaRendezvous>(
        self_idx: usize,
        group_connections: &mut [&'a mut T],
        distance: usize,
    ) -> Result<(), SyncError> {
        let conn = group_connections
            .get_mut(Self::add_mod(self_idx, distance, group_connections.len()))
            .unwrap();

        conn.rendezvous()?;

        Ok(())
    }

    fn rendezvous_left<'a, T: 'a + RdmaSendRecv + RdmaRendezvous>(
        self_idx: usize,
        group_connections: &mut [&'a mut T],
        distance: usize,
    ) -> Result<(), SyncError> {
        let conn = group_connections
            .get_mut(Self::sub_mod(self_idx, distance, group_connections.len()))
            .unwrap();

        conn.rendezvous()?;

        Ok(())
    }

    fn add_mod(a: usize, b: usize, m: usize) -> usize {
        (a + b) % m
    }

    fn sub_mod(a: usize, b: usize, m: usize) -> usize {
        (a + m - b) % m
    }
}
