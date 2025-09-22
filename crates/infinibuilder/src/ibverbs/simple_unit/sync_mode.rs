use crate::connection::Connect;
use crate::ibverbs::simple_unit::connection::{IbvConnection, UnconnectedIbvConnection};
use crate::ibverbs::simple_unit::mode::Mode;
use crate::ibverbs::work_request::CachedWorkRequest;
use crate::rdma_traits::{RdmaRendezvous, WorkRequest};
use crate::unsafe_slice::UnsafeSlice;
use ibverbs::{MemoryRegion, RemoteMemoryRegion};
use std::ops::{Deref, RangeInclusive};
use std::time::Duration;
use crate::ibverbs::simple_unit::IbvSimpleUnit;

pub struct SyncMode;

impl Mode for SyncMode {
    type UnconnectedMr = UnconnectedSyncMr;
    type ConnectedMr = ConnectedSyncMr;
    type MrConnectionConfig = SyncMrConnectionConfig;
}

pub struct UnconnectedSyncMr {
    rendezvous_state: Box<RendezvousMemoryRegion>,
    rendezvous_mr: MemoryRegion<UnsafeSlice<RendezvousState>>,
}

impl UnconnectedSyncMr {
    pub fn new(connection: &mut UnconnectedIbvConnection) -> std::io::Result<Self> {
        let rendezvous_state = Box::new(RendezvousMemoryRegion::new());
        let rendezvous_mr = connection
            .pd
            .register(unsafe { UnsafeSlice::new(rendezvous_state.as_ref()) })?;
        Ok(Self {
            rendezvous_state,
            rendezvous_mr,
        })
    }
}

impl Connect for UnconnectedSyncMr {
    type ConnectionConfig = SyncMrConnectionConfig;
    type Connected = ConnectedSyncMr;

    fn connection_config(&self) -> Self::ConnectionConfig {
        SyncMrConnectionConfig {
            remote_rendezvous_mr: self.rendezvous_mr.remote(),
        }
    }

    fn connect(
        self,
        connection_config: Self::ConnectionConfig,
    ) -> std::io::Result<Self::Connected> {
        Ok(ConnectedSyncMr {
            rendezvous_state: self.rendezvous_state,
            rendezvous_mr: self.rendezvous_mr,
            remote_rendezvous_mr: connection_config.remote_rendezvous_mr,
        })
    }
}

pub struct SyncMrConnectionConfig {
    remote_rendezvous_mr: RemoteMemoryRegion,
}

pub struct ConnectedSyncMr {
    rendezvous_state: Box<RendezvousMemoryRegion>,
    rendezvous_mr: MemoryRegion<UnsafeSlice<RendezvousState>>,
    remote_rendezvous_mr: RemoteMemoryRegion,
}

impl ConnectedSyncMr {
    pub(super) fn rendezvous(&mut self, connection: &mut IbvConnection) -> std::io::Result<()> {
        let wr_id = connection.fetch_advance_wr_id();

        // Write READY to the peer's rendezvous memory
        connection.qp.post_write(
            &[self
                .rendezvous_mr
                .slice(self.rendezvous_state.local_state_range())],
            self.remote_rendezvous_mr
                .slice(self.rendezvous_state.remote_state_range()),
            wr_id,
            None,
        )?;
        CachedWorkRequest::<1>::new(wr_id, connection.cached_cq.clone()).wait()?;

        // Wait for peer to write on our rendezvous memory
        while let RendezvousState::Waiting = self.rendezvous_state.remote_state() {
            std::hint::spin_loop();
        }

        // Reset our rendezvous memory so the operation can be repeated
        self.rendezvous_state.reset_remote_state();

        Ok(())
    }
}

impl RdmaRendezvous for IbvSimpleUnit<SyncMode> {
    fn rendezvous(&mut self) -> std::io::Result<()> {
        self.mr.rendezvous(&mut self.connection)
    }

    fn rendezvous_timeout(&mut self, timeout: Duration) -> std::io::Result<()> {
        todo!()
    }
}

#[repr(u8)]
#[derive(Debug, Default, Copy, Clone)]
enum RendezvousState {
    #[default]
    Waiting,
    Ready,
}

#[repr(transparent)]
#[derive(Debug)]
struct RendezvousMemoryRegion([RendezvousState; 2]);

impl RendezvousMemoryRegion {
    const LOCAL_IDX: usize = 0;
    const REMOTE_IDX: usize = 1;

    fn new() -> Self {
        Self([RendezvousState::Ready, RendezvousState::Waiting])
    }

    fn remote_state(&self) -> RendezvousState {
        self.0[Self::REMOTE_IDX]
    }

    fn reset_remote_state(&mut self) {
        self.0[Self::REMOTE_IDX] = RendezvousState::Waiting;
    }

    fn remote_state_range(&self) -> RangeInclusive<usize> {
        Self::REMOTE_IDX..=Self::REMOTE_IDX
    }

    fn local_state_range(&self) -> RangeInclusive<usize> {
        Self::LOCAL_IDX..=Self::LOCAL_IDX
    }
}

impl Deref for RendezvousMemoryRegion {
    type Target = [RendezvousState];

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}
