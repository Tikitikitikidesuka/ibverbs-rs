use crate::channel::raw_channel::RawChannel;
use crate::ibverbs::memory_region::MemoryRegion;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::ibverbs::remote_memory_region::RemoteMemoryRegion;
use crate::ibverbs::work_request::WriteWorkRequest;
use std::fmt::Debug;
use std::io;
use std::mem::offset_of;
use std::sync::atomic::{Ordering, fence};
use std::time::Duration;
use zerocopy::network_endian::{U32, U64};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

pub struct MetaMr {
    memory: Box<MetaMrState>,
    mr: MemoryRegion,
    remote_mr: RemoteMemoryRegion,
}

pub struct PreparedMetaMr {
    memory: Box<MetaMrState>,
    mr: MemoryRegion,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, FromBytes, IntoBytes, Immutable, KnownLayout)]
struct MetaMrState {
    in_remote_mr: PodRemoteMemoryRegion,
    in_epoch: U64,
    in_ack: U64,
    out_remote_mr: PodRemoteMemoryRegion,
    out_epoch: U64,
    out_ack: U64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, FromBytes, IntoBytes, Immutable, KnownLayout)]
struct PodRemoteMemoryRegion {
    addr: U64,
    length: U64,
    rkey: U32,
    _pad: U32,
}

// Native -> Big Endian
impl From<RemoteMemoryRegion> for PodRemoteMemoryRegion {
    fn from(value: RemoteMemoryRegion) -> Self {
        PodRemoteMemoryRegion {
            addr: U64::new(value.addr as u64),
            length: U64::new(value.length as u64),
            rkey: U32::new(value.rkey),
            _pad: U32::new(0),
        }
    }
}

// Big Endian -> Native
impl From<PodRemoteMemoryRegion> for RemoteMemoryRegion {
    fn from(value: PodRemoteMemoryRegion) -> Self {
        RemoteMemoryRegion {
            addr: value.addr.get() as usize,
            length: value.length.get() as usize,
            rkey: value.rkey.get(),
        }
    }
}

impl PodRemoteMemoryRegion {
    fn new() -> Self {
        Self {
            addr: U64::new(0),
            length: U64::new(0),
            rkey: U32::new(0),
            _pad: U32::new(0),
        }
    }
}

impl PreparedMetaMr {
    pub fn remote(&self) -> RemoteMemoryRegion {
        self.mr.remote()
    }

    pub fn link_remote(self, remote_mr: RemoteMemoryRegion) -> MetaMr {
        MetaMr {
            memory: self.memory,
            mr: self.mr,
            remote_mr,
        }
    }
}

impl MetaMr {
    pub fn new(pd: &ProtectionDomain) -> io::Result<PreparedMetaMr> {
        let mut memory = Box::new(MetaMrState {
            in_remote_mr: PodRemoteMemoryRegion::new(),
            in_epoch: U64::new(0),
            in_ack: U64::new(0),
            out_remote_mr: PodRemoteMemoryRegion::new(),
            out_epoch: U64::new(0),
            out_ack: U64::new(0),
        });

        let mr = unsafe {
            pd.register_shared_mr(
                memory.as_mut() as *mut MetaMrState as *mut u8,
                size_of::<MetaMrState>(),
            )?
        };

        Ok(PreparedMetaMr { memory, mr })
    }

    /// Protocol for sharing a remote memory region.
    /// 1. Write the remote memory region to the meta mr with `set_remote_mr`.
    /// 2. Advance the epoch with `increase_remote_mr_epoch`.
    /// 3. RDMA write the wr from `prepare_write_remote_mr_wr`.
    /// This writes the remote mr to the peer.
    /// 4. RDMA write the wr from `prepare_write_remote_mr_epoch_wr`.
    /// This advances the epoch to the peer, notifying him that he can read
    /// the previously sent memory region. These two cannot be done in a single
    /// work request because there is no guarantee that the operation is atomic.
    /// However, a reliable channel like RawChannel guarantees that multiple WRs
    /// are seen in order of issuance.
    pub fn share_memory_region(
        &mut self,
        channel: &mut RawChannel,
        mr: &MemoryRegion,
    ) -> io::Result<()> {
        // 0. Check the peer acknowledged the last shared remote mr (be -> native)
        let current_in_ack = unsafe { std::ptr::read_volatile(&self.memory.in_ack) }.get();
        if self.memory.out_epoch > current_in_ack {
            return Err(io::Error::new(
                io::ErrorKind::ResourceBusy,
                "Peer has not acknowledged a previously shared memory region",
            ));
        }

        // 1. Write the mr's remote handle to the outgoing remote mr field
        self.memory.out_remote_mr = mr.remote().into();

        // 2. Increase the epoch in the outgoing remote mr epoch field (native -> +1 -> be)
        let new_epoch = self.memory.out_epoch.get() + 1;
        self.memory.out_epoch.set(new_epoch);

        // Slice the meta remote memory region
        // Unwrap because we are taking the full slice
        let mut meta_remote_mr_slice = self.remote_mr.slice_mut(..).unwrap();
        let (mut in_remote_mr, mut rest) =
            meta_remote_mr_slice.split_at_mut(size_of::<PodRemoteMemoryRegion>());
        let (mut in_epoch, _rest) = rest.split_at_mut(size_of::<u64>());

        // 3. Prepare RDMA write request of the remote mr
        // Get slice of the outgoing remote mr field's bytes
        let out_remote_mr_bytes = self.memory.out_remote_mr.as_bytes();
        // Unwrap because the bytes are guaranteed to be in the mr and fit in a sge.
        let remote_mr_sges = [self.mr.prepare_gather_element(out_remote_mr_bytes).unwrap()];
        let remote_mr_wr = WriteWorkRequest::new(&remote_mr_sges, &mut in_remote_mr);

        // 4. Prepare RDMA write request of the remote mr epoch
        // Get slice of the remote mr epoch field's bytes
        let out_epoch_bytes = self.memory.out_epoch.as_bytes();
        // Unwrap because the bytes are guaranteed to be in the mr and fit in a sge.
        let remote_mr_epoch_sges = [self.mr.prepare_gather_element(out_epoch_bytes).unwrap()];
        let remote_mr_epoch_wr = WriteWorkRequest::new(&remote_mr_epoch_sges, &mut in_epoch); // Ensure changes are visible before issuing the writes

        fence(Ordering::Release);

        // 5. Post RDMA write operations in the correct order:
        // - Firstly write the remote mr.
        // - Secondly write the increased epoch.
        channel
            .scope(|s| {
                let remote_mr_wr = s.post_write(remote_mr_wr)?;
                let epoch_wr = s.post_write(remote_mr_epoch_wr)?;
                remote_mr_wr.spin_poll()?;
                epoch_wr.spin_poll()?;
                Ok::<(), io::Error>(())
            })
            .expect("Implementation error: All wrs polled manually in the scope")?;

        Ok(())
    }

    /// Protocol for receive a shared memory region
    /// 1. Wait until the epoch is one value higher than the ack.
    /// 2. Read the remote memory region.
    /// 3. Acknowledge it by adding one to the ack and RDMA writing the wr generated from
    /// `prepare_write_ack_remote_mr_wr` to the peer that shared the mr.
    pub fn accept_memory_region(
        &mut self,
        channel: &mut RawChannel,
        timeout: Duration,
    ) -> io::Result<RemoteMemoryRegion> {
        let start = std::time::Instant::now();

        loop {
            // 0. Poll Epoch (be -> native)
            let current_in_epoch = unsafe { std::ptr::read_volatile(&self.memory.in_epoch) }.get();

            // 1. Wait until the incoming epoch is higher than the one acknowledged
            if current_in_epoch > self.memory.out_ack.get() {
                // 2. Read the remote memory region
                let remote_mr = self.memory.in_remote_mr.into();

                // 3. Increase the outgoing acknowledge counter (native -> +1 -> be)
                let new_epoch = self.memory.out_ack.get() + 1;
                self.memory.out_ack.set(new_epoch);

                // Slice the meta remote memory region
                // Unwrap because we are taking the full slice
                let ack_offset = offset_of!(MetaMrState, in_ack);
                let mut meta_remote_mr_slice = self
                    .remote_mr
                    .slice_mut(ack_offset..ack_offset + size_of::<u64>())
                    .unwrap();

                // Get slice of the remote mr ack field's bytes
                let remote_mr_ack_sge = [self
                    .mr
                    .prepare_gather_element(self.memory.out_ack.as_bytes())
                    .unwrap()];

                // 4. Prepare RDMA write request of the remote mr ack
                let wr = WriteWorkRequest::new(remote_mr_ack_sge, &mut meta_remote_mr_slice);

                // Ensure change is visible before issuing the write
                fence(Ordering::Release);

                // 5. Post RDMA write operation to acknowledge.
                channel.write(wr)?;

                return Ok(remote_mr);
            }

            if start.elapsed() > timeout {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Timed out accepting shared memory region",
                ));
            }

            std::hint::spin_loop()
        }
    }

    /*
    pub fn set_remote_mr() {
        todo!()
    }

    /// Returns None if there peer still has not acknowledged a previous request (not ready)
    pub fn prepare_write_remote_mr_wr(
        &'_ mut self,
        remote_mr: RemoteMemoryRegion,
    ) -> Option<WriteWorkRequest<'_, Vec<GatherElement<'_>>, RemoteMemorySliceMut<'_>>> {
        // Load with Acquire to sync with any previous writes
        let ack = self.memory.in_remote_mr_ack.load(Ordering::Acquire);
        let current_epoch = self.memory.out_remote_mr_epoch.load(Ordering::Relaxed);

        if current_epoch > ack {
            return None;
        }

        // Write Payload
        // Since we are the only local writer to `out_remote_mr`, this is safe without atomics.
        self.memory.out_remote_mr = MaybeUninit::new(remote_mr);

        // Increment Epoch with Release ordering
        // This acts as a fence: ensures the `out_remote_mr` write above
        // is visible before the epoch update is visible.
        let new_epoch = current_epoch + 1;
        self.memory
            .out_remote_mr_epoch
            .store(new_epoch, Ordering::Release);

        let mr_bytes = unsafe {
            slice::from_raw_parts(
                self.memory.out_remote_mr.as_ptr() as *const u8,
                size_of::<RemoteMemoryRegion>(),
            )
        };

        let epoch_bytes = unsafe {
            slice::from_raw_parts(
                &self.memory.out_remote_mr_epoch as *const AtomicUsize as *const u8,
                size_of::<usize>(),
            )
        };

        let sge_mr = self
            .mr
            .prepare_gather_element(mr_bytes)
            .expect("Invariant violation: `out_remote_mr` is not within the registered `mr`");
        let sge_epoch = self.mr.prepare_gather_element(epoch_bytes).expect(
            "Invariant violation: `local_remote_mr_epoch` is not within the registered `mr`",
        );

        let offset = offset_of!(MetaMrState, in_remote_mr);
        let len = size_of::<RemoteMemoryRegion>() + size_of::<usize>();
        let range = offset..offset + len;
        let remote_slice = self.remote_mr.slice_mut(range).expect(
            "Invariant violation: Remote MR is too small to contain `in_remote_mr` and epoch",
        );

        Some(WriteWorkRequest::new(vec![sge_mr, sge_epoch], remote_slice))
    }

    pub fn read_remote_mr(&self) -> Option<RemoteMemoryRegion> {
        // Load Epoch with Acquire
        // Ensures we see the data payload written before this epoch was updated.
        let epoch = self.memory.in_remote_mr_epoch.load(Ordering::Acquire);
        let ack = self.memory.out_remote_mr_ack.load(Ordering::Relaxed);

        if epoch > ack {
            // SAFE: Acquire ordering above guarantees `in_remote_mr` is valid/updated
            Some(unsafe { self.memory.in_remote_mr.assume_init() })
        } else {
            None
        }
    }

    /// Returns None if there is no remote mr to acknowledge
    pub fn prepare_write_ack_remote_mr_wr(
        &'_ mut self,
    ) -> Option<WriteWorkRequest<'_, Vec<GatherElement<'_>>, RemoteMemorySliceMut<'_>>> {
        let epoch = self.memory.in_remote_mr_epoch.load(Ordering::Acquire);
        let ack = self.memory.out_remote_mr_ack.load(Ordering::Relaxed);

        if ack < epoch {
            // Increment Ack
            let new_ack = ack + 1;
            // Store with Release implies the read of the data is "done"
            // before we tell the peer we are done.
            self.memory
                .out_remote_mr_ack
                .store(new_ack, Ordering::Release);

            let ack_bytes = unsafe {
                slice::from_raw_parts(
                    &self.memory.out_remote_mr_ack as *const AtomicUsize as *const u8,
                    size_of::<usize>(),
                )
            };

            let sge_ack = self.mr.prepare_gather_element(ack_bytes).expect(
                "Invariant violation: `peer_remote_mr_ack` is not within the registered `mr`",
            );
            let offset = offset_of!(MetaMrState, in_remote_mr_ack);
            let len = size_of::<usize>();
            let range = offset..offset + len;
            let remote_slice = self
                .remote_mr
                .slice_mut(range)
                .expect("Invariant violation: Remote MR too small for `local_remote_mr_ack`");

            Some(WriteWorkRequest::new(vec![sge_ack], remote_slice))
        } else {
            None
        }
    }
    */

    /*
    pub fn increase_sync_epoch(&mut self) {
        self.memory.out_sync_epoch += 1;
    }

    pub fn get_sync_epoch(&self) -> usize {
        self.memory.in_sync_epoch
    }

    pub fn prepare_sync_epoch_wr(
        &mut self,
    ) -> WriteWorkRequest<Vec<GatherElement>, RemoteMemorySliceMut> {
        // Ensure previous modifications are visible to NIC
        fence(Ordering::Release);

        // Write from out_sync
        let out_sync_bytes: &[u8] = unsafe {
            slice::from_raw_parts(
                &self.memory.out_sync_epoch as *const usize as *const u8,
                size_of::<u32>(),
            )
        };

        // To in_sync
        let offset = offset_of!(MetaMrState, in_sync_epoch);
        let range = offset..offset + size_of::<usize>();

        WriteWorkRequest::new(
            vec![self.mr.prepare_gather_element(out_sync_bytes).unwrap()],
            self.remote_mr.slice_mut(range).unwrap(),
        )
    }
    */
}

/*
#[derive(Debug, Copy, Clone)]
pub enum MetaMessageView {
    SharedMemoryRegion(MemoryRegionEndpoint),
    Unknown(u32),
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct MetaMessage {
    pub tag: u32,
    pub _pad: u32,
    pub payload: MetaMessagePayload,
}

#[repr(C)]
#[derive(Copy, Clone, Zeroable)]
pub union MetaMessagePayload {
    pub shared_mr: MemoryRegionEndpoint,
}

// SAFETY:
// 1. The union is `repr(C)`.
// 2. All fields (MemoryRegionEndpoint) are Pod.
// 3. Use of MetaMessage tag for safe logical access.
unsafe impl Pod for MetaMessagePayload {}

impl MetaMessage {
    pub const TAG_SHARED_MR: u32 = 1;

    pub fn view(&self) -> MetaMessageView {
        // SAFETY: Only access the union field if the tag matches known variants.
        unsafe {
            match self.tag {
                Self::TAG_SHARED_MR => MetaMessageView::SharedMemoryRegion(self.payload.shared_mr),
                t => MetaMessageView::Unknown(t),
            }
        }
    }

    pub fn set(&mut self, val: MetaMessageView) {
        match val {
            MetaMessageView::SharedMemoryRegion(mr) => {
                self.tag = Self::TAG_SHARED_MR;
                self.payload.shared_mr = mr;
            }
            MetaMessageView::Unknown(t) => {
                self.tag = t;
                // Leave payload as is
            }
        }
    }
}

impl Debug for MetaMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.view())
    }
}
 */
