use crate::ibverbs::completion_queue::CompletionQueueInner;
use crate::ibverbs::prepared_queue_pair::PreparedQueuePair;
use crate::ibverbs::protection_domain::ProtectionDomainInner;
use crate::ibverbs::queue_pair::QueuePair;
use ibverbs_sys::*;
use std::ffi::c_void;
use std::sync::Arc;
use std::time::Duration;
use std::{io, ptr};

pub struct QueuePairBuilder {
    pd: Arc<ProtectionDomainInner>,
    send_cq: Arc<CompletionQueueInner>,
    recv_cq: Arc<CompletionQueueInner>,

    /// Maximum simultaneous issued send work requests.
    max_send_wr: u32,
    /// Maximum number of scatter-gather elements per send work request.
    max_send_sge: u32,
    /// Maximum simultaneous issued receive work requests.
    max_recv_wr: u32,
    /// Maximum number of scatter-gather elements per receive work request.
    max_recv_sge: u32,
    /// Encoding specifying flags for the queue pair.
    access: ibv_access_flags,
    /// Max number of retries when receiver reports it is not ready.
    max_rnr_retries: u8,
    /// Max number of retries when receiver does not reply.
    max_ack_retries: u8,
    /// Encoding specifying the timeout between retries when receiver reports it is not ready.
    min_rnr_timer: u8,
    /// Encoding specifying the timeout between retries for receiver to reply.
    ack_timeout: u8,
}

impl QueuePairBuilder {
    const DEFAULT_MAX_WR: u32 = 16;
    const DEFAULT_MAX_SGE: u32 = 16;
    const DEFAULT_MAX_RNR_RETRIES: u8 = 6;
    const DEFAULT_MAX_ACK_RETRIES: u8 = 6;
    const DEFAULT_RNR_TIMEOUT: u8 = 16;
    const DEFAULT_ACK_TIMEOUT: u8 = 4;
    const DEFAULT_ACCESS_FLAGS: ibv_access_flags = ibv_access_flags::IBV_ACCESS_LOCAL_WRITE;

    pub fn new(
        pd: Arc<ProtectionDomainInner>,
        send_cq: Arc<CompletionQueueInner>,
        recv_cq: Arc<CompletionQueueInner>,
    ) -> Self {
        Self {
            pd,
            send_cq,
            recv_cq,
            max_send_wr: Self::DEFAULT_MAX_WR,
            max_send_sge: Self::DEFAULT_MAX_SGE,
            max_recv_wr: Self::DEFAULT_MAX_WR,
            max_recv_sge: Self::DEFAULT_MAX_SGE,
            access: Self::DEFAULT_ACCESS_FLAGS,
            max_rnr_retries: Self::DEFAULT_MAX_RNR_RETRIES,
            max_ack_retries: Self::DEFAULT_MAX_ACK_RETRIES,
            min_rnr_timer: Self::DEFAULT_RNR_TIMEOUT,
            ack_timeout: Self::DEFAULT_ACK_TIMEOUT,
        }
    }

    /// # Errors
    ///  - `EINVAL`: Invalid `ProtectionDomain`, sending or receiving `Context`, or invalid value
    ///    provided in `max_send_wr`, `max_recv_wr`, or in `max_inline_data`.
    ///  - `ENOMEM`: Not enough resources to complete this operation.
    ///  - `ENOSYS`: QP with this Transport Service Type isn't supported by this RDMA device.
    ///  - `EPERM`: Not enough permissions to create a QP with this Transport Service Type.
    pub fn build(&self) -> io::Result<PreparedQueuePair> {
        let mut attr = ibv_qp_init_attr {
            qp_context: ptr::null::<c_void>() as *mut _,
            send_cq: self.send_cq.cq as *const _ as *mut _,
            recv_cq: self.recv_cq.cq as *const _ as *mut _,
            srq: ptr::null::<ibv_srq>() as *mut _,
            cap: ibv_qp_cap {
                max_send_wr: self.max_send_wr,
                max_recv_wr: self.max_recv_wr,
                max_send_sge: self.max_send_sge,
                max_recv_sge: self.max_recv_sge,
                max_inline_data: 0,
            },
            qp_type: ibv_qp_type::IBV_QPT_RC,
            sq_sig_all: 0,
        };

        let qp = unsafe { ibv_create_qp(self.pd.pd, &mut attr as *mut _) };
        if qp.is_null() {
            Err(io::Error::last_os_error())
        } else {
            log::debug!("IbvQueuePair created");
            Ok(PreparedQueuePair {
                qp: QueuePair {
                    pd: self.pd.clone(),
                    qp,
                },
                lid: self.pd.context.query_port()?.lid,

                max_send_wr: self.max_send_wr,
                max_send_sge: self.max_send_sge,
                max_recv_wr: self.max_recv_wr,
                max_recv_sge: self.max_recv_sge,
                access: self.access,
                max_rnr_retries: self.max_rnr_retries,
                max_ack_retries: self.max_ack_retries,
                min_rnr_timer: self.min_rnr_timer,
                ack_timeout: self.ack_timeout,
            })
        }
    }

    pub fn with_max_send_wrs(&mut self, max_send_wr: u32) -> &mut Self {
        self.max_send_wr = max_send_wr;
        self
    }

    pub fn with_max_send_sges(&mut self, max_send_sge: u32) -> &mut Self {
        self.max_send_sge = max_send_sge;
        self
    }

    pub fn with_max_recv_wrs(&mut self, max_recv_wr: u32) -> &mut Self {
        self.max_recv_wr = max_recv_wr;
        self
    }

    pub fn with_max_recv_sges(&mut self, max_recv_sge: u32) -> &mut Self {
        self.max_recv_sge = max_recv_sge;
        self
    }

    pub fn with_access_flags(&mut self, access_flags: AccessFlags) -> &mut Self {
        self.access = access_flags.inner;
        self
    }

    pub fn with_min_rnr_timer(&mut self, timer: RnrTimer) -> &mut Self {
        self.min_rnr_timer = timer.code();
        self
    }

    pub fn with_ack_timeout(&mut self, timeout: AckTimeout) -> &mut Self {
        self.ack_timeout = timeout.code();
        self
    }

    pub fn with_max_ack_retries(&mut self, retries: AckRetries) -> &mut Self {
        self.max_ack_retries = retries.code();
        self
    }

    pub fn with_max_rnr_retries(&mut self, retries: RnrRetries) -> &mut Self {
        self.max_rnr_retries = retries.code();
        self
    }
}

pub struct AccessFlags {
    inner: ibv_access_flags,
}

impl AccessFlags {
    /// New access flags with no flags set
    pub fn new() -> Self {
        Self {
            inner: ibv_access_flags(0),
        }
    }

    pub fn with_local_write(mut self) -> Self {
        self.inner |= ibv_access_flags::IBV_ACCESS_LOCAL_WRITE;
        self
    }

    pub fn with_remote_read(mut self) -> Self {
        self.inner |= ibv_access_flags::IBV_ACCESS_REMOTE_READ;
        self
    }

    pub fn with_remote_write(mut self) -> Self {
        self.inner |= ibv_access_flags::IBV_ACCESS_REMOTE_WRITE;
        self
    }
}

impl From<AccessFlags> for ibv_access_flags {
    fn from(value: AccessFlags) -> Self {
        value.inner
    }
}

pub struct PacketSequenceNumber(u32);

impl PacketSequenceNumber {
    pub fn new(psn: u32) -> Option<Self> {
        (psn < 1 << 24).then_some(Self(psn))
    }
}

impl From<PacketSequenceNumber> for u32 {
    fn from(value: PacketSequenceNumber) -> Self {
        value.0
    }
}

pub enum MaximumTransferUnit {
    MTU256 = 1,
    MTU512 = 2,
    MTU1024 = 3,
    MTU2048 = 4,
    MTU4096 = 5,
}

impl From<MaximumTransferUnit> for ibv_mtu {
    fn from(value: MaximumTransferUnit) -> Self {
        value as ibv_mtu
    }
}

/// Minimum RNR NAK Timer Field Value. When an incoming message to this QP should
/// consume a Work Request from the Receive Queue, but not Work Request is outstanding
/// on that Queue, the QP will send an RNR NAK packet to the initiator.
/// It does not affect RNR NAKs sent for other reasons.
/// From [RDMAMojo](https://www.rdmamojo.com/2013/01/12/ibv_modify_qp/).
pub struct RnrTimer(u8);

impl RnrTimer {
    /// Value 0 encodes a duration of 655.36 ms
    const DURATION_ZERO: Duration = Duration::from_secs(655360);

    /// Durations corresponding to values 1, 2, 3, ... up to 31.
    /// Index 0 of this slice corresponds to code 1, index 1 to code 2, etc. (index = code - 1)
    /// From page 333 of [InfiniBandTM Architecture Specification Volume 1 Release 1.2.1](https://www.afs.enea.it/asantoro/V1r1_2_1.Release_12062007.pdf).
    const DURATION_TABLE: [Duration; 31] = [
        Duration::from_micros(10),     // 0.01 ms
        Duration::from_micros(20),     // 0.02 ms
        Duration::from_micros(30),     // 0.03 ms
        Duration::from_micros(40),     // 0.04 ms
        Duration::from_micros(60),     // 0.06 ms
        Duration::from_micros(80),     // 0.08 ms
        Duration::from_micros(120),    // 0.12 ms
        Duration::from_micros(160),    // 0.16 ms
        Duration::from_micros(240),    // 0.24 ms
        Duration::from_micros(320),    // 0.32 ms
        Duration::from_micros(480),    // 0.48 ms
        Duration::from_micros(640),    // 0.64 ms
        Duration::from_micros(960),    // 0.96 ms
        Duration::from_micros(1280),   // 1.28 ms
        Duration::from_micros(1920),   // 1.92 ms
        Duration::from_micros(2560),   // 2.56 ms
        Duration::from_micros(3840),   // 3.84 ms
        Duration::from_micros(5120),   // 5.12 ms
        Duration::from_micros(7680),   // 7.68 ms
        Duration::from_micros(10240),  // 10.24 ms
        Duration::from_micros(15360),  // 15.36 ms
        Duration::from_micros(20480),  // 20.48 ms
        Duration::from_micros(30720),  // 30.72 ms
        Duration::from_micros(40960),  // 40.96 ms
        Duration::from_micros(61440),  // 61.44 ms
        Duration::from_micros(81920),  // 81.92 ms
        Duration::from_micros(122880), // 122.88 ms
        Duration::from_micros(163840), // 163.84 ms
        Duration::from_micros(245760), // 245.76 ms
        Duration::from_micros(327680), // 327.68 ms
        Duration::from_micros(491520), // 491.52 ms
    ];

    pub fn limited(code: u8) -> Option<Self> {
        (code > 0 && code < 32).then_some(RnrTimer(code))
    }

    pub fn min_duration_greater_than(timeout: Duration) -> Self {
        RnrTimer(match Self::DURATION_TABLE.binary_search(&timeout) {
            Ok(idx) => (idx + 1) as u8, // Exact match found
            Err(idx) if idx < Self::DURATION_TABLE.len() => (idx + 1) as u8, // Min greater
            _ => 0,                     // Zero encodes greatest duration
        })
    }

    pub fn duration(&self) -> Duration {
        if self.0 > 0 {
            *Self::DURATION_TABLE
                .get(self.0 as usize - 1)
                .expect("rnr_timeout cannot be greater than 31")
        } else {
            Self::DURATION_ZERO
        }
    }

    pub fn code(&self) -> u8 {
        self.0
    }
}

/// The minimum timeout that a QP waits for ACK/NACK from remote QP before
/// retransmitting the packet. The value zero is special value which means
/// wait an infinite time for the ACK/NACK (useful for debugging).
/// For any other value of timeout, the time calculation is: 4.096*2^timeout usec.
/// From [RDMAMojo](https://www.rdmamojo.com/2013/01/12/ibv_modify_qp/).
pub enum AckTimeout {
    Limited(u8),
    Unlimited,
}

impl AckTimeout {
    pub fn limited(code: u8) -> Option<Self> {
        (code > 0 && code < 32).then_some(AckTimeout::Limited(code))
    }

    pub fn unlimited() -> Self {
        AckTimeout::Unlimited
    }

    pub fn min_duration_greater_than(timeout: Duration) -> Option<Self> {
        let code = (timeout.as_micros() / 4096).next_power_of_two().ilog2();
        (code > 0 && code < 32).then_some(AckTimeout::Limited(code as u8))
    }

    pub fn duration(&self) -> Option<Duration> {
        match self {
            AckTimeout::Limited(code) => Some(Duration::from_micros(4096u64 << code)),
            AckTimeout::Unlimited => None,
        }
    }

    pub fn code(&self) -> u8 {
        match self {
            AckTimeout::Limited(code) => *code,
            AckTimeout::Unlimited => 0,
        }
    }
}

/// A 3 bits value of the total number of times that the QP will try to resend the
/// packets when an RNR NACK was sent by the remote QP before reporting an error.
/// The value 7 is special and specify to retry infinite times in case of RNR.
/// From [RDMAMojo](https://www.rdmamojo.com/2013/01/12/ibv_modify_qp/).
pub enum RnrRetries {
    Limited(u8),
    Unlimited,
}

impl RnrRetries {
    pub fn limited(retries: u8) -> Option<Self> {
        (retries < 7).then_some(RnrRetries::Limited(retries))
    }

    pub fn unlimited() -> Self {
        RnrRetries::Unlimited
    }

    pub fn retries(&self) -> Option<u8> {
        match self {
            RnrRetries::Limited(retries) => Some(*retries),
            RnrRetries::Unlimited => None,
        }
    }

    pub fn code(&self) -> u8 {
        self.retries().unwrap_or(7)
    }
}

/// A 3 bits value of the total number of times that the QP will try to resend the
/// packets before reporting an error because the remote side doesn't answer in the primary path
/// From [RDMAMojo](https://www.rdmamojo.com/2013/01/12/ibv_modify_qp/).
pub struct AckRetries(u8);

impl AckRetries {
    pub fn limited(retries: u8) -> Option<Self> {
        (retries <= 7).then_some(AckRetries(retries))
    }

    pub fn retries(&self) -> u8 {
        self.0
    }

    pub fn code(&self) -> u8 {
        self.retries()
    }
}
