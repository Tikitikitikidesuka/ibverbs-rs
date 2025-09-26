use crate::connect::Connect;
use crate::ibverbs::simple_unit::IbvSimpleUnit;
use crate::ibverbs::simple_unit::connection::{IbvConnection, UnconnectedIbvConnection};
use crate::ibverbs::simple_unit::mode::Mode;
use crate::ibverbs::unsafe_slice::UnsafeSlice;
use crate::ibverbs::work_request::CachedWorkRequest;
use crate::rdma_traits::{RdmaRendezvous, WorkRequest};
use ibverbs::{MemoryRegion, RemoteMemoryRegion};
use serde::{Deserialize, Serialize};
use std::ops::{Deref, Range};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct SyncMode;

impl Mode for SyncMode {
    type UnconnectedMr = UnconnectedSyncMr;
    type ConnectedMr = ConnectedSyncMr;
    type MrConnectionConfig = SyncMrConnectionConfig;
}

pub struct UnconnectedSyncMr {
    rendezvous_state: Box<RendezvousMemoryRegion>,
    rendezvous_mr: MemoryRegion<UnsafeSlice<u64>>,
}

impl UnconnectedSyncMr {
    pub fn new(connection: &mut UnconnectedIbvConnection) -> std::io::Result<Self> {
        // Box to ensure stable location in heap memory for DMA
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

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncMrConnectionConfig {
    remote_rendezvous_mr: RemoteMemoryRegion,
}

pub struct ConnectedSyncMr {
    rendezvous_state: Box<RendezvousMemoryRegion>,
    rendezvous_mr: MemoryRegion<UnsafeSlice<u64>>,
    remote_rendezvous_mr: RemoteMemoryRegion,
}

impl ConnectedSyncMr {
    pub(super) fn rendezvous<const POLL_BUFF_SIZE: usize>(
        &mut self,
        connection: &mut IbvConnection,
    ) -> std::io::Result<()> {
        // Increment local generation
        self.rendezvous_state.advance_epoch();

        // Write next_epoch to peer
        let wr_id = connection.cached_cq.fetch_advance_next_wr_id();
        connection.qp.post_write(
            &[self
                .rendezvous_mr
                .slice(self.rendezvous_state.local_epoch_mr_range())],
            self.remote_rendezvous_mr
                .slice(self.rendezvous_state.remote_epoch_mr_range()),
            wr_id,
            None,
        )?;
        CachedWorkRequest::<POLL_BUFF_SIZE>::new(wr_id, connection.cached_cq.clone()).wait()?;

        // Wait for peer to write their next_epoch
        while !self.rendezvous_state.remote_synced() {
            std::hint::spin_loop();
        }

        Ok(())
    }

    pub(super) fn rendezvous_timeout<const POLL_BUFF_SIZE: usize>(
        &mut self,
        connection: &mut IbvConnection,
        timeout: Duration,
    ) -> std::io::Result<()> {
        // Get start time
        let init_time = Instant::now();

        // Increment local generation
        self.rendezvous_state.advance_epoch();

        // Write next_epoch to peer
        let wr_id = connection.cached_cq.fetch_advance_next_wr_id();
        connection.qp.post_write(
            &[self
                .rendezvous_mr
                .slice(self.rendezvous_state.local_epoch_mr_range())],
            self.remote_rendezvous_mr
                .slice(self.rendezvous_state.remote_epoch_mr_range()),
            wr_id,
            None,
        )?;

        // Wait send for timeout - elapsed
        CachedWorkRequest::<POLL_BUFF_SIZE>::new(wr_id, connection.cached_cq.clone())
            .wait_timeout(timeout - init_time.elapsed())?;

        // Wait for peer to write their next_epoch
        while !self.rendezvous_state.remote_synced() {
            // Return if timeout
            if init_time.elapsed() > timeout {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "Timed out",
                ));
            }

            std::hint::spin_loop();
        }

        Ok(())
    }
}

impl RdmaRendezvous for IbvSimpleUnit<SyncMode> {
    fn rendezvous(&mut self) -> std::io::Result<()> {
        self.mr.rendezvous::<1>(&mut self.connection)
    }

    fn rendezvous_timeout(&mut self, timeout: Duration) -> std::io::Result<()> {
        self.mr
            .rendezvous_timeout::<1>(&mut self.connection, timeout)
    }
}

#[repr(transparent)]
#[derive(Debug)]
struct RendezvousMemoryRegion([u64; 2]);

impl RendezvousMemoryRegion {
    const LOCAL_IDX: usize = 0;
    const REMOTE_IDX: usize = 1;

    pub fn new() -> Self {
        Self([0; 2])
    }

    pub fn advance_epoch(&mut self) {
        self.0[Self::LOCAL_IDX] += 1;
    }

    pub fn remote_synced(&self) -> bool {
        self.0[Self::REMOTE_IDX] >= self.0[Self::LOCAL_IDX]
    }

    pub fn remote_epoch_mr_range(&self) -> Range<usize> {
        Self::REMOTE_IDX * size_of::<u64>()..(Self::REMOTE_IDX + 1) * size_of::<u64>()
    }

    pub fn local_epoch_mr_range(&self) -> Range<usize> {
        Self::LOCAL_IDX * size_of::<u64>()..(Self::LOCAL_IDX + 1) * size_of::<u64>()
    }
}

impl Deref for RendezvousMemoryRegion {
    type Target = [u64];

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}
