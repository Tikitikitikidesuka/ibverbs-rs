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
use std::ptr::{read, read_volatile, write_volatile};
use std::time::{Duration, Instant};

#[derive(Debug, Copy, Clone)]
pub struct SyncMode;

impl Mode for SyncMode {
    type UnconnectedMr = UnconnectedSyncMr;
    type ConnectedMr = ConnectedSyncMr;
    type MrConnectionConfig = SyncMrConnectionConfig;
}

pub struct UnconnectedSyncMr {
    state: Box<RendezvousState>,
    mr: MemoryRegion,
}

impl UnconnectedSyncMr {
    pub fn new(connection: &mut UnconnectedIbvConnection) -> std::io::Result<Self> {
        // Box to ensure stable location in heap memory for DMA
        let mut state = Box::new(RendezvousState::new());
        let state_ptr = &mut state.raw as *mut u8;
        let state_length = size_of::<RendezvousState>();
        let mr = connection.pd.register(state_ptr, state_length)?;
        Ok(Self { state, mr })
    }
}

impl Connect for UnconnectedSyncMr {
    type ConnectionConfig = SyncMrConnectionConfig;
    type Connected = ConnectedSyncMr;

    fn connection_config(&self) -> Self::ConnectionConfig {
        SyncMrConnectionConfig {
            remote_rendezvous_mr: self.mr.remote(),
        }
    }

    fn connect(
        self,
        connection_config: Self::ConnectionConfig,
    ) -> std::io::Result<Self::Connected> {
        Ok(ConnectedSyncMr {
            rendezvous_state: self.state,
            rendezvous_mr: self.mr,
            remote_rendezvous_mr: connection_config.remote_rendezvous_mr,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncMrConnectionConfig {
    remote_rendezvous_mr: RemoteMemoryRegion,
}

pub struct ConnectedSyncMr {
    rendezvous_state: Box<RendezvousState>,
    rendezvous_mr: MemoryRegion,
    remote_rendezvous_mr: RemoteMemoryRegion,
}

impl ConnectedSyncMr {
    pub(super) fn is_peer_waiting(&self) -> bool {
        self.rendezvous_state.is_remote_waiting()
    }

    pub(super) fn wait_for_peer_signal(&self) -> std::io::Result<()> {
        while !self.is_peer_waiting() {
            //std::hint::spin_loop();
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

            //std::hint::spin_loop();
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
            //std::hint::spin_loop();
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

            //std::hint::spin_loop();
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
struct RendezvousState {
    raw: [u8; 2 * size_of::<u64>()],
}

impl RendezvousState {
    const LOCAL_BYTE_IDX: usize = 0 * size_of::<u64>();
    const REMOTE_BYTE_IDX: usize = 1 * size_of::<u64>();

    pub fn new() -> Self {
        Self {
            raw: [0u8; 2 * size_of::<u64>()],
        }
    }

    #[inline(always)]
    pub fn local_epoch_mr_range(&self) -> Range<usize> {
        Self::LOCAL_BYTE_IDX..Self::LOCAL_BYTE_IDX + size_of::<u64>()
    }

    #[inline(always)]
    pub fn remote_epoch_mr_range(&self) -> Range<usize> {
        Self::REMOTE_BYTE_IDX..Self::REMOTE_BYTE_IDX + size_of::<u64>()
    }

    #[inline(always)]
    pub fn advance_epoch(&mut self) {
        // Non volatile read since only we modify it
        let epoch = unsafe { read(self.raw.as_ptr().add(Self::LOCAL_BYTE_IDX) as *const u64) };
        // Volatile read since the hardware must know it has been changed for rdma write
        unsafe {
            write_volatile(
                self.raw.as_mut_ptr().add(Self::LOCAL_BYTE_IDX) as *mut u64,
                epoch + 1,
            )
        };
    }

    #[inline(always)]
    fn is_remote_ahead(&self) -> bool {
        // Non volatile read since only we modify it
        let local = unsafe { read(self.raw.as_ptr().add(Self::LOCAL_BYTE_IDX) as *const u64) };
        // Volatile read since it gets written into by the peer
        let remote =
            unsafe { read_volatile(self.raw.as_ptr().add(Self::REMOTE_BYTE_IDX) as *mut u64) };
        remote >= local
    }

    #[inline(always)]
    pub fn is_remote_waiting(&self) -> bool {
        // Non volatile read since only we modify it
        let local = unsafe { read(self.raw.as_ptr().add(Self::LOCAL_BYTE_IDX) as *const u64) };
        // Volatile read since it gets written into by the peer
        let remote =
            unsafe { read_volatile(self.raw.as_ptr().add(Self::REMOTE_BYTE_IDX) as *mut u64) };
        remote > local
    }
}

impl Deref for RendezvousState {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.raw.as_ref()
    }
}
