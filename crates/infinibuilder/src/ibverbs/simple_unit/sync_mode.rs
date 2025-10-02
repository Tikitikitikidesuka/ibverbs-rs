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
use std::pin::Pin;
use std::ptr::{read_volatile, write_volatile};
use std::time::{Duration, Instant};

#[derive(Debug, Copy, Clone)]
pub struct SyncMode;

impl Mode for SyncMode {
    type UnconnectedMr = UnconnectedSyncMr;
    type ConnectedMr = ConnectedSyncMr;
    type MrConnectionConfig = SyncMrConnectionConfig;
}

pub struct UnconnectedSyncMr {
    rendezvous_state: Pin<Box<RendezvousMemoryRegion>>,
    rendezvous_mr: MemoryRegion<UnsafeSlice<u64>>,
}

impl UnconnectedSyncMr {
    pub fn new(connection: &mut UnconnectedIbvConnection) -> std::io::Result<Self> {
        // Box to ensure stable location in heap memory for DMA
        let rendezvous_state = Box::pin(RendezvousMemoryRegion::new());
        let rendezvous_mr = connection
            .pd
            .register(unsafe { UnsafeSlice::new(&*rendezvous_state) })?;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncMrConnectionConfig {
    remote_rendezvous_mr: RemoteMemoryRegion,
}

pub struct ConnectedSyncMr {
    rendezvous_state: Pin<Box<RendezvousMemoryRegion>>,
    rendezvous_mr: MemoryRegion<UnsafeSlice<u64>>,
    remote_rendezvous_mr: RemoteMemoryRegion,
}

impl ConnectedSyncMr {
    pub(super) fn is_peer_waiting(&self) -> bool {
        self.rendezvous_state.is_remote_waiting()
    }

    pub(super) fn wait_for_peer_signal(&self) -> std::io::Result<()> {
        while !self.is_peer_waiting() {
            std::hint::spin_loop();
        }

        Ok(())
    }

    pub(super) fn wait_for_peer_signal_timeout(&self, timeout: Duration) -> std::io::Result<()> {
        // Get start time
        let init_time = Instant::now();

        while !self.is_peer_waiting() {
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

        // Wait for peer to be synced
        while !self.rendezvous_state.is_remote_ahead() {
            std::hint::spin_loop();
        }

        Ok(())
    }

    pub(super) fn rendezvous_timeout<const POLL_BUFF_SIZE: usize>(
        &mut self,
        connection: &mut IbvConnection,
        timeout: Duration,
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

        // Get start time
        let init_time = Instant::now();

        // Wait write for timeout
        CachedWorkRequest::<POLL_BUFF_SIZE>::new(wr_id, connection.cached_cq.clone())
            .wait_timeout(timeout)?;

        // Wait for peer to be synced
        while !self.rendezvous_state.is_remote_ahead() {
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
    fn is_peer_waiting(&self) -> bool {
        self.mr.is_peer_waiting()
    }

    fn wait_for_peer_signal(&self) -> std::io::Result<()> {
        self.mr.wait_for_peer_signal()
    }

    fn wait_for_peer_signal_timeout(&self, timeout: Duration) -> std::io::Result<()> {
        self.mr.wait_for_peer_signal_timeout(timeout)
    }

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
        Self([0, 0])
    }

    pub fn advance_epoch(&mut self) {
        let value = self.0[Self::LOCAL_IDX];
        unsafe { write_volatile(self.0.as_mut_ptr(), value + 1) };
    }

    fn is_remote_ahead(&self) -> bool {
        let local = self.0[Self::LOCAL_IDX];
        let remote = unsafe { read_volatile(self.0.as_ptr().add(1)) };
        remote >= local
    }

    pub fn is_remote_waiting(&self) -> bool {
        let local = self.0[Self::LOCAL_IDX];
        let remote = unsafe { read_volatile(self.0.as_ptr().add(1)) };
        remote > local
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
