use crate::ibverbs::cached_cq::CachedCompletionQueue;
use crate::ibverbs::work_request::CachedWorkRequest;
use crate::rdma_traits::{RdmaReadWrite, RdmaRendezvous, RdmaSendRecv, WorkRequest};
use crate::unsafe_slice::UnsafeSlice;
use derivative::Derivative;
use ibverbs::{
    CompletionQueue, MemoryRegion, PreparedQueuePair, ProtectionDomain, QueuePair,
    QueuePairEndpoint, RemoteMemoryRegion, ibv_access_flags, ibv_qp_type, ibv_wc, ibv_wc_status,
};
use std::ops::{Deref, Range, RangeBounds};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

pub trait Mode {
    type Unconnected;
    type Connected;
    type ConnectionConfig;

    fn connection_config(unconnected: &Self::Unconnected) -> Self::ConnectionConfig;

    fn connect(
        unconnected: Self::Unconnected,
        connection_config: Self::ConnectionConfig,
    ) -> Self::Connected;
}

impl Mode for TransferMode {
    type Unconnected = UnconnectedTransferModeMr;
    type Connected = ConnectedTransferModeMr;
    type ConnectionConfig = TransferModeConnectionMr;

    fn connection_config(unconnected: &Self::Unconnected) -> Self::ConnectionConfig {
        TransferModeConnectionMr {
            remote_mr: unconnected.mr.remote(),
        }
    }

    fn connect(
        unconnected: Self::Unconnected,
        endpoint: Self::ConnectionConfig,
    ) -> Self::Connected {
        ConnectedTransferModeMr {
            mr: unconnected.mr,
            remote_mr: endpoint.remote_mr,
        }
    }
}

impl Mode for SyncMode {
    type Unconnected = UnconnectedSyncModeMr;
    type Connected = ConnectedSyncModeMr;
    type ConnectionConfig = SyncModeConnectionMr;

    fn connection_config(unconnected: &Self::Unconnected) -> Self::ConnectionConfig {
        SyncModeConnectionMr {
            remote_rendezvous_mr: unconnected.rendezvous_mr.remote(),
        }
    }

    fn connect(
        unconnected: Self::Unconnected,
        connection_config: Self::ConnectionConfig,
    ) -> Self::Connected {
        ConnectedSyncModeMr {
            rendezvous_state: unconnected.rendezvous_state,
            rendezvous_mr: unconnected.rendezvous_mr,
            remote_rendezvous_mr: connection_config.remote_rendezvous_mr,
        }
    }
}

impl Mode for SyncTransferMode {
    type Unconnected = UnconnectedSyncTransferModeMr;
    type Connected = ConnectedSyncTransferModeMr;
    type ConnectionConfig = SyncTransferModeConnectionMr;

    fn connection_config(unconnected: &Self::Unconnected) -> Self::ConnectionConfig {
        SyncTransferModeConnectionMr {
            transfer_connection_mr: TransferMode::connection_config(&unconnected.transfer_mem),
            sync_connection_mr: SyncMode::connection_config(&unconnected.sync_mem),
        }
    }

    fn connect(
        unconnected: Self::Unconnected,
        connection_config: Self::ConnectionConfig,
    ) -> Self::Connected {
        ConnectedSyncTransferModeMr {
            transfer_mem: TransferMode::connect(
                unconnected.transfer_mem,
                connection_config.transfer_connection_mr,
            ),
            sync_mem: SyncMode::connect(unconnected.sync_mem, connection_config.sync_connection_mr),
        }
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct UnconnectedIbSimpleUnit<const CQ_SIZE: usize, M: Mode> {
    #[derivative(Debug = "ignore")]
    prepared_qp: PreparedQueuePair,
    qp_endpoint: QueuePairEndpoint,
    #[derivative(Debug = "ignore")]
    mode_mr: M::Unconnected,
    #[derivative(Debug = "ignore")]
    pd: ProtectionDomain,
    #[derivative(Debug = "ignore")]
    cq: CompletionQueue,
}

#[derive(Derivative)]
#[derivative(Debug)]
struct UnconnectedTransferModeMr {
    #[derivative(Debug = "ignore")]
    mr: MemoryRegion<UnsafeSlice<u8>>,
}

#[derive(Derivative)]
#[derivative(Debug)]
struct UnconnectedSyncModeMr {
    #[derivative(Debug = "ignore")]
    rendezvous_state: Box<RendezvousMemoryRegion>,
    #[derivative(Debug = "ignore")]
    rendezvous_mr: MemoryRegion<UnsafeSlice<RendezvousState>>,
}

#[derive(Debug)]
struct UnconnectedSyncTransferModeMr {
    transfer_mem: UnconnectedTransferModeMr,
    sync_mem: UnconnectedSyncModeMr,
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct IbSimpleUnit<const CQ_SIZE: usize, M: Mode> {
    self_qp_endpoint: QueuePairEndpoint,
    remote_qp_endpoint: QueuePairEndpoint,
    #[derivative(Debug = "ignore")]
    qp: QueuePair,
    #[derivative(Debug = "ignore")]
    mode_mr: M::Connected,
    #[derivative(Debug = "ignore")]
    pd: ProtectionDomain,
    #[derivative(Debug = "ignore")]
    cached_cq: Arc<CachedCompletionQueue<CQ_SIZE>>,
    #[derivative(Debug = "ignore")]
    next_wr_id: AtomicU64,
}

#[derive(Derivative)]
#[derivative(Debug)]
struct ConnectedTransferModeMr {
    #[derivative(Debug = "ignore")]
    mr: MemoryRegion<UnsafeSlice<u8>>,
    #[derivative(Debug = "ignore")]
    remote_mr: RemoteMemoryRegion,
}

#[derive(Derivative)]
#[derivative(Debug)]
struct ConnectedSyncModeMr {
    #[derivative(Debug = "ignore")]
    rendezvous_state: Box<RendezvousMemoryRegion>,
    #[derivative(Debug = "ignore")]
    rendezvous_mr: MemoryRegion<UnsafeSlice<RendezvousState>>,
    #[derivative(Debug = "ignore")]
    remote_rendezvous_mr: RemoteMemoryRegion,
}

#[derive(Debug)]
struct ConnectedSyncTransferModeMr {
    transfer_mem: ConnectedTransferModeMr,
    sync_mem: ConnectedSyncModeMr,
}

pub struct TransferMode;
pub struct SyncMode;
pub struct SyncTransferMode;

impl<const CQ_SIZE: usize, M: Mode> UnconnectedIbSimpleUnit<CQ_SIZE, M> {
    pub fn connection_config(&self) -> ConnectionEndpoint<M> {
        ConnectionEndpoint {
            qp_endpoint: self.qp_endpoint,
            connection_mode_mr: M::connection_config(&self.mode_mr),
        }
    }

    pub fn connect(
        self,
        connection_config: ConnectionEndpoint<M>,
    ) -> std::io::Result<IbSimpleUnit<CQ_SIZE, M>> {
        Ok(IbSimpleUnit {
            self_qp_endpoint: self.qp_endpoint,
            remote_qp_endpoint: connection_config.qp_endpoint,
            qp: self.prepared_qp.handshake(connection_config.qp_endpoint)?,
            mode_mr: M::connect(self.mode_mr, connection_config.connection_mode_mr),
            pd: self.pd,
            cached_cq: Arc::new(CachedCompletionQueue::new(self.cq)),
            next_wr_id: AtomicU64::new(0),
        })
    }
}

pub struct TransferModeConnectionMr {
    remote_mr: RemoteMemoryRegion,
}

pub struct SyncModeConnectionMr {
    remote_rendezvous_mr: RemoteMemoryRegion,
}

pub struct SyncTransferModeConnectionMr {
    transfer_connection_mr: TransferModeConnectionMr,
    sync_connection_mr: SyncModeConnectionMr,
}

pub struct ConnectionEndpoint<M: Mode> {
    qp_endpoint: QueuePairEndpoint,
    connection_mode_mr: M::ConnectionConfig,
}

impl IbSimpleUnit<0> {
    /// SAFETY: Memory slice will have its ownership unlinked, meaning that it might be freed but this
    /// struct will still hold a reference to it which could result in illegal accesses to memory and UB.
    /// Memory is also taken as immutable reference, however by the nature of RDMA it is aliased and therefore
    /// can be mutated regardless.
    pub unsafe fn new_sync_transfer_unit<const CQ_SIZE: usize>(
        ib_context: &ibverbs::Context,
        memory: &[u8],
    ) -> std::io::Result<UnconnectedIbSimpleUnit<CQ_SIZE, SyncTransferMode>> {
        let cq = ib_context.create_cq(CQ_SIZE as i32, 0)?;
        let pd = ib_context.alloc_pd()?;
        let mr = pd.register(unsafe { UnsafeSlice::new(memory) })?;
        let rendezvous_state = Box::new(RendezvousMemoryRegion::new());
        let rendezvous_mr = pd.register(unsafe { UnsafeSlice::new(rendezvous_state.as_ref()) })?;
        let prepared_qp = pd
            .create_qp(&cq, &cq, ibv_qp_type::IBV_QPT_RC)?
            .set_access(
                ibv_access_flags::IBV_ACCESS_REMOTE_WRITE
                    | ibv_access_flags::IBV_ACCESS_REMOTE_READ
                    | ibv_access_flags::IBV_ACCESS_LOCAL_WRITE,
            )
            .build()?;
        let qp_endpoint = prepared_qp.endpoint()?;

        Ok(UnconnectedIbSimpleUnit {
            prepared_qp,
            qp_endpoint,
            mr,
            rendezvous_state,
            rendezvous_mr,
            pd,
            cq,
        })
    }
}

impl<const CQ_SIZE: usize> RdmaSendRecv for IbSimpleUnit<CQ_SIZE> {
    /// # SAFETY
    /// The memory region can only be safely reused or dropped after the request is fully executed
    /// and a work completion has been retrieved from the corresponding completion queue
    /// (i.e., until CompletionQueue::poll returns a completion for this send).
    unsafe fn post_send(
        &mut self,
        mr_range: impl RangeBounds<usize>,
        imm_data: Option<u32>,
    ) -> Result<impl WorkRequest, std::io::Error> {
        let wr_id = self.next_wr_id.fetch_add(1, Ordering::Relaxed);
        unsafe {
            self.qp
                .post_send(&[self.mr.slice(mr_range)], wr_id, imm_data)
        }?;
        Ok(CachedWorkRequest::new(wr_id, self.cached_cq.clone()))
    }

    /// # SAFETY
    /// The memory region can only be safely reused or dropped after the request is fully executed
    /// and a work completion has been retrieved from the corresponding completion queue
    /// (i.e., until CompletionQueue::poll returns a completion for this receive)
    unsafe fn post_receive(
        &mut self,
        mr_range: impl RangeBounds<usize>,
    ) -> Result<impl WorkRequest, std::io::Error> {
        let wr_id = self.next_wr_id.fetch_add(1, Ordering::Relaxed);
        unsafe { self.qp.post_receive(&[self.mr.slice(mr_range)], wr_id) }?;
        Ok(CachedWorkRequest::new(wr_id, self.cached_cq.clone()))
    }
}

impl<const CQ_SIZE: usize> RdmaReadWrite for IbSimpleUnit<CQ_SIZE> {
    /// TODO: WRITE SAFETY
    unsafe fn post_write(
        &mut self,
        mr_range: impl RangeBounds<usize>,
        remote_mr_range: impl RangeBounds<usize>,
        imm_data: Option<u32>,
    ) -> Result<impl WorkRequest, std::io::Error> {
        let wr_id = self.next_wr_id.fetch_add(1, Ordering::Relaxed);
        self.qp.post_write(
            &[self.mr.slice(mr_range)],
            self.remote_mr.slice(remote_mr_range),
            wr_id,
            imm_data,
        )?;
        Ok(CachedWorkRequest::new(wr_id, self.cached_cq.clone()))
    }

    /// TODO: WRITE SAFETY
    unsafe fn post_read(
        &mut self,
        mr_range: impl RangeBounds<usize>,
        remote_mr_slice: impl RangeBounds<usize>,
    ) -> Result<impl WorkRequest, std::io::Error> {
        let wr_id = self.next_wr_id.fetch_add(1, Ordering::Relaxed);
        self.qp.post_read(
            &[self.mr.slice(mr_range)],
            self.remote_mr.slice(remote_mr_slice),
            wr_id,
        )?;
        Ok(CachedWorkRequest::new(wr_id, self.cached_cq.clone()))
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
    fn new() -> Self {
        Self([RendezvousState::Ready, RendezvousState::Waiting])
    }

    fn remote_state(&self) -> RendezvousState {
        self.0[1]
    }

    fn reset_remote_state(&mut self) {
        self.0[1] = RendezvousState::Waiting;
    }

    fn remote_state_range(&self) -> Range<usize> {
        1..2
    }

    fn local_state_range(&self) -> Range<usize> {
        0..1
    }
}

impl Deref for RendezvousMemoryRegion {
    type Target = [RendezvousState];

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl<const CQ_SIZE: usize> RdmaRendezvous for IbSimpleUnit<CQ_SIZE> {
    fn rendezvous(&mut self) -> std::io::Result<()> {
        // Write READY to the peer's rendezvous memory
        let wr_id = self.next_wr_id.fetch_add(1, Ordering::Relaxed);
        self.qp.post_write(
            &[self
                .rendezvous_mr
                .slice(self.rendezvous_state.local_state_range())],
            self.remote_rendezvous_mr
                .slice(self.rendezvous_state.remote_state_range()),
            wr_id,
            None,
        )?;
        CachedWorkRequest::new(wr_id, self.cached_cq.clone()).wait()?;

        // Wait for peer to write on our rendezvous memory
        while let RendezvousState::Waiting = self.rendezvous_state.remote_state() {
            std::hint::spin_loop();
        }

        // Reset our rendezvous memory so the operation can be repeated
        self.rendezvous_state.reset_remote_state();

        Ok(())
    }

    fn rendezvous_timeout(&mut self, timeout: Duration) -> std::io::Result<()> {
        todo!()
    }
}
