use crate::connect::Connect;
use crate::ibverbs::simple_unit::connection::UnconnectedIbvConnection;
use crate::ibverbs::simple_unit::mode::Mode;
use ibverbs::MemoryRegion;
use serde::{Deserialize, Serialize};
use std::ptr::{read, read_volatile, write, write_volatile};

#[derive(Debug, Copy, Clone)]
pub struct SyncTransferMode;

impl Mode for SyncTransferMode {
    type UnconnectedMr = UnconnectedSyncTransferMr;
    type ConnectedMr = ConnectedSyncTransferMr;
    type MrConnectionConfig = SyncTransferMrConnectionConfig;
}

pub struct UnconnectedSyncTransferMr {
    state: Box<SyncTransferState>,
    mr: MemoryRegion,
}

impl UnconnectedSyncTransferMr {
    pub fn new(connection: &mut UnconnectedIbvConnection) -> std::io::Result<Self> {
        // Box to ensure stable location in heap memory for DMA
        let mut state = Box::new(SyncTransferState::new());
        let state_ptr = &mut state.raw as *mut u8;
        let state_length = size_of::<SyncTransferState>();
        let mr = connection.pd.register(state_ptr, state_length)?;
        Ok(Self { state, mr })
    }
}

impl Connect for UnconnectedSyncTransferMr {
    type ConnectionConfig = SyncTransferMrConnectionConfig;
    type Connected = ConnectedSyncTransferMr;

    fn connection_config(&self) -> Self::ConnectionConfig {
        todo!()
    }

    fn connect(
        self,
        connection_config: Self::ConnectionConfig,
    ) -> std::io::Result<Self::Connected> {
        todo!()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncTransferMrConnectionConfig {}

pub struct ConnectedSyncTransferMr {}

#[derive(Debug)]
#[repr(transparent)]
pub struct SyncTransferState {
    raw: [u8; 3 * size_of::<u64>()],
    // Raw data of:
    // send_tokens: u64,
    // issued_sends: u64,
    // issued_recvs: u64,
}

impl SyncTransferState {
    const SEND_TOKENS_BYTE_IDX: usize = 0 * size_of::<u64>();
    const ISSUED_SENDS_BYTE_IDX: usize = 1 * size_of::<u64>();
    const ISSUED_RECVS_BYTE_IDX: usize = 2 * size_of::<u64>();

    pub fn new() -> Self {
        Self {
            raw: [0u8; 3 * size_of::<u64>()],
        }
    }

    #[inline(always)]
    pub fn send_tokens(&self) -> u64 {
        // Read volatile since it gets rdma written into by peer
        unsafe { read_volatile(self.raw.as_ptr().add(Self::SEND_TOKENS_BYTE_IDX) as *const u64) }
    }

    #[inline(always)]
    pub fn issued_sends(&self) -> u64 {
        // Non volatile since only self writes to it
        unsafe { read(self.raw.as_ptr().add(Self::ISSUED_SENDS_BYTE_IDX) as *const u64) }
    }

    #[inline(always)]
    pub fn issued_recvs(&self) -> u64 {
        // Non volatile since only self writes to it
        unsafe { read(self.raw.as_ptr().add(Self::ISSUED_RECVS_BYTE_IDX) as *const u64) }
    }

    #[inline(always)]
    pub fn issue_send(&mut self) {
        // Non volatile since only self reads it
        unsafe {
            write(
                self.raw.as_ptr().add(Self::ISSUED_SENDS_BYTE_IDX) as *mut u64,
                self.issued_sends() + 1,
            )
        }
    }

    #[inline(always)]
    pub fn issue_recv(&mut self) {
        // Volatile since its written to the peer through rdma
        unsafe {
            write_volatile(
                self.raw.as_mut_ptr().add(Self::ISSUED_RECVS_BYTE_IDX) as *mut u64,
                self.issued_recvs() + 1,
            )
        }
    }
}

impl AsMut<[u8]> for SyncTransferState {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.raw
    }
}
