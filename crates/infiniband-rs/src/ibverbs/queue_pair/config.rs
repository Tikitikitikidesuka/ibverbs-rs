use std::time::Duration;

#[derive(Debug, Copy, Clone)]
pub struct PacketSequenceNumber(u32);

impl PacketSequenceNumber {
    pub const fn new(psn: u32) -> Option<Self> {
        if psn < (1 << 24) {
            Some(Self(psn))
        } else {
            None
        }
    }

    pub const fn code(&self) -> u32 {
        self.0
    }
}

impl Default for PacketSequenceNumber {
    fn default() -> PacketSequenceNumber {
        PacketSequenceNumber(0)
    }
}

#[derive(Debug, Copy, Clone)]
pub enum MaximumTransferUnit {
    MTU256 = 1,
    MTU512 = 2,
    MTU1024 = 3,
    MTU2048 = 4,
    MTU4096 = 5,
}

impl MaximumTransferUnit {
    pub const fn code(&self) -> u8 {
        *self as u8
    }
}

impl Default for MaximumTransferUnit {
    fn default() -> MaximumTransferUnit {
        MaximumTransferUnit::MTU4096
    }
}

/// Minimum RNR NAK Timer Field Value. When an incoming message to this QP should
/// consume a Work Request from the Receive Queue, but not Work Request is outstanding
/// on that Queue, the QP will send an RNR NAK packet to the initiator.
/// It does not affect RNR NAKs sent for other reasons.
/// From [RDMAMojo](https://www.rdmamojo.com/2013/01/12/ibv_modify_qp/).
#[derive(Debug, Copy, Clone)]
pub struct MinRnrTimer(u8);

impl MinRnrTimer {
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

    pub const fn limited(code: u8) -> Option<Self> {
        if code > 0 && code < 32 {
            Some(MinRnrTimer(code))
        } else {
            None
        }
    }

    pub fn min_duration_greater_than(timeout: Duration) -> Self {
        MinRnrTimer(match Self::DURATION_TABLE.binary_search(&timeout) {
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

    pub const fn code(&self) -> u8 {
        self.0
    }
}

impl Default for MinRnrTimer {
    fn default() -> MinRnrTimer {
        MinRnrTimer(16)
    }
}

/// A 3 bits value of the total number of times that the QP will try to resend the
/// packets when an RNR NACK was sent by the remote QP before reporting an error.
/// The value 7 is special and specify to retry infinite times in case of RNR.
/// From [RDMAMojo](https://www.rdmamojo.com/2013/01/12/ibv_modify_qp/).
#[derive(Debug, Copy, Clone)]
pub enum MaxRnrRetries {
    Limited(u8),
    Unlimited,
}

impl MaxRnrRetries {
    pub const fn limited(retries: u8) -> Option<Self> {
        if retries < 7 {
            Some(MaxRnrRetries::Limited(retries))
        } else {
            None
        }
    }

    pub const fn unlimited() -> Self {
        MaxRnrRetries::Unlimited
    }

    pub const fn retries(&self) -> Option<u8> {
        match self {
            MaxRnrRetries::Limited(retries) => Some(*retries),
            MaxRnrRetries::Unlimited => None,
        }
    }

    pub const fn code(&self) -> u8 {
        match self {
            MaxRnrRetries::Limited(retries) => *retries,
            MaxRnrRetries::Unlimited => 7,
        }
    }
}

impl Default for MaxRnrRetries {
    fn default() -> MaxRnrRetries {
        MaxRnrRetries::Limited(6)
    }
}

/// The minimum timeout that a QP waits for ACK/NACK from remote QP before
/// retransmitting the packet. The value zero is special value which means
/// wait an infinite time for the ACK/NACK (useful for debugging).
/// For any other value of timeout, the time calculation is: 4.096*2^timeout usec.
/// From [RDMAMojo](https://www.rdmamojo.com/2013/01/12/ibv_modify_qp/).
#[derive(Debug, Copy, Clone)]
pub enum AckTimeout {
    Limited(u8),
    Unlimited,
}

impl AckTimeout {
    pub const fn limited(code: u8) -> Option<Self> {
        if code > 0 && code < 32 {
            Some(AckTimeout::Limited(code))
        } else {
            None
        }
    }

    pub const fn unlimited() -> Self {
        AckTimeout::Unlimited
    }

    pub const fn min_duration_greater_than(timeout: Duration) -> Option<Self> {
        let code = (timeout.as_micros() / 4096).next_power_of_two().ilog2();
        if code > 0 && code < 32 {
            Some(AckTimeout::Limited(code as u8))
        } else {
            None
        }
    }

    pub fn duration(&self) -> Option<Duration> {
        match self {
            AckTimeout::Limited(code) => Some(Duration::from_micros(4096u64 << code)),
            AckTimeout::Unlimited => None,
        }
    }

    pub const fn code(&self) -> u8 {
        match self {
            AckTimeout::Limited(code) => *code,
            AckTimeout::Unlimited => 0,
        }
    }
}

impl Default for AckTimeout {
    fn default() -> AckTimeout {
        AckTimeout::Limited(4)
    }
}

/// A 3 bits value of the total number of times that the QP will try to resend the
/// packets before reporting an error because the remote side doesn't answer in the primary path
/// From [RDMAMojo](https://www.rdmamojo.com/2013/01/12/ibv_modify_qp/).
#[derive(Debug, Copy, Clone)]
pub struct MaxAckRetries(u8);

impl MaxAckRetries {
    pub const fn limited(retries: u8) -> Option<Self> {
        if retries <= 7 {
            Some(MaxAckRetries(retries))
        } else {
            None
        }
    }

    pub const fn retries(&self) -> u8 {
        self.0
    }

    pub const fn code(&self) -> u8 {
        self.retries()
    }
}

impl Default for MaxAckRetries {
    fn default() -> MaxAckRetries {
        MaxAckRetries(6)
    }
}
