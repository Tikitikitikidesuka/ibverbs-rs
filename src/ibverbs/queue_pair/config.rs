use std::time::Duration;

/// A 24-bit Packet Sequence Number (PSN).
#[derive(Debug, Copy, Clone, Default)]
pub struct PacketSequenceNumber(u32);

impl PacketSequenceNumber {
    /// Creates a new PSN. Returns `None` if the value exceeds the 24-bit limit (`0xFFFFFF`).
    pub const fn new(psn: u32) -> Option<Self> {
        if psn < (1 << 24) {
            Some(Self(psn))
        } else {
            None
        }
    }

    /// Returns the raw 24-bit value.
    pub const fn code(&self) -> u32 {
        self.0
    }
}

/// The Maximum Transfer Unit (MTU) for the path.
#[derive(Debug, Copy, Clone, Default)]
pub enum MaximumTransferUnit {
    MTU256 = 1,
    MTU512 = 2,
    MTU1024 = 3,
    MTU2048 = 4,
    #[default]
    MTU4096 = 5,
}

impl MaximumTransferUnit {
    /// Returns the ibverbs code for this MTU.
    pub const fn code(&self) -> u8 {
        *self as u8
    }
}

/// Minimum RNR NAK Timer Field Value.
///
/// When an incoming message arrives but no Receive WQE is posted, the QP sends an
/// RNR NAK (Receiver Not Ready) to the sender. This timer tells the sender how long
/// to wait before retrying.
///
/// See [RDMAMojo](https://www.rdmamojo.com/2013/01/12/ibv_modify_qp/) for details.
#[derive(Debug, Copy, Clone)]
pub struct MinRnrTimer(u8);

impl MinRnrTimer {
    /// Value 0 encodes the maximum duration (approx 655 ms).
    const DURATION_ZERO: Duration = Duration::from_secs(655360);

    /// Lookup table for codes 1..31 mapping to durations in microseconds.
    /// Derived from InfiniBand Architecture Specification Vol 1.
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

    /// Creates a timer from a raw 5-bit code (1-31). Returns `None` if out of range.
    pub const fn limited(code: u8) -> Option<Self> {
        if code > 0 && code < 32 {
            Some(MinRnrTimer(code))
        } else {
            None
        }
    }

    /// Finds the smallest RNR timer code that represents a duration greater than `timeout`.
    // binary_search on a 31-element table yields idx ≤ 30, so idx+1 ≤ 31, fits in u8
    #[allow(clippy::cast_possible_truncation)]
    pub fn min_duration_greater_than(timeout: Duration) -> Self {
        MinRnrTimer(match Self::DURATION_TABLE.binary_search(&timeout) {
            Ok(idx) => (idx + 1) as u8, // Exact match found
            Err(idx) if idx < Self::DURATION_TABLE.len() => (idx + 1) as u8, // Min greater
            _ => 0,                     // Zero encodes greatest duration
        })
    }

    /// Returns the approximate duration represented by this timer code.
    pub fn duration(&self) -> Duration {
        if self.0 > 0 {
            *Self::DURATION_TABLE
                .get(self.0 as usize - 1)
                .expect("rnr_timeout cannot be greater than 31")
        } else {
            Self::DURATION_ZERO
        }
    }

    /// Returns the ibverbs code for this `MinRnrTimer`.
    pub const fn code(&self) -> u8 {
        self.0
    }
}

impl Default for MinRnrTimer {
    fn default() -> MinRnrTimer {
        MinRnrTimer(16)
    }
}

/// Configures how many times the sender should retry after receiving an RNR NACK.
///
/// If the receiver is busy (no WQEs posted), it sends an RNR NACK. This setting controls
/// how many times the sender retries before giving up and reporting an error.
#[derive(Debug, Copy, Clone)]
pub enum MaxRnrRetries {
    /// Retry a specific number of times (0-6).
    Limited(u8),
    /// Retry infinitely until the receiver posts a WQE.
    Unlimited,
}

impl MaxRnrRetries {
    /// Retry `retries` times. Returns `None` if `retries > 6`.
    pub const fn limited(retries: u8) -> Option<Self> {
        if retries < 7 {
            Some(MaxRnrRetries::Limited(retries))
        } else {
            None
        }
    }

    /// Retry forever.
    pub const fn unlimited() -> Self {
        MaxRnrRetries::Unlimited
    }

    /// Returns the number of retries.
    pub const fn retries(&self) -> Option<u8> {
        match self {
            MaxRnrRetries::Limited(retries) => Some(*retries),
            MaxRnrRetries::Unlimited => None,
        }
    }

    /// Returns the ibverbs code for this `MaxRnrRetries`.
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

/// Configures the transport-level Acknowledgement Timeout.
///
/// This determines how long the QP waits for an ACK from the remote peer before
/// retransmitting a packet.
///
/// Formula: `4.096 microseconds * 2^timeout`.
#[derive(Debug, Copy, Clone)]
pub enum AckTimeout {
    /// Timeout code (1-31).
    Limited(u8),
    /// Wait infinitely (useful for debugging, prevents retransmission).
    Unlimited,
}

impl AckTimeout {
    /// Creates a timeout from a raw code (1-31).
    pub const fn limited(code: u8) -> Option<Self> {
        if code > 0 && code < 32 {
            Some(AckTimeout::Limited(code))
        } else {
            None
        }
    }

    /// Wait infinitely (code 0).
    pub const fn unlimited() -> Self {
        AckTimeout::Unlimited
    }

    /// Calculates the smallest timeout code that covers the given `timeout` duration.
    // code is checked to be in 1..31 before the cast
    #[allow(clippy::cast_possible_truncation)]
    pub const fn min_duration_greater_than(timeout: Duration) -> Option<Self> {
        let code = (timeout.as_micros() / 4096).next_power_of_two().ilog2();
        if code > 0 && code < 32 {
            Some(AckTimeout::Limited(code as u8))
        } else {
            None
        }
    }

    /// Returns the approximate duration for this timeout.
    pub fn duration(&self) -> Option<Duration> {
        match self {
            AckTimeout::Limited(code) => Some(Duration::from_micros(4096u64 << code)),
            AckTimeout::Unlimited => None,
        }
    }

    /// Returns the ibverbs code for this `AckTimeout`.
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

/// Configures the number of transport-level retransmissions.
///
/// If an ACK is not received within [`AckTimeout`], the QP retransmits.
/// This controls how many times it retries before declaring the connection broken.
#[derive(Debug, Copy, Clone)]
pub struct MaxAckRetries(u8);

impl MaxAckRetries {
    /// Sets the number of retries (0-7).
    pub const fn limited(retries: u8) -> Option<Self> {
        if retries <= 7 {
            Some(MaxAckRetries(retries))
        } else {
            None
        }
    }

    /// Returns the number of retries.
    pub const fn retries(&self) -> u8 {
        self.0
    }

    /// Returns the ibverbs code for this `MaxAckRetries`.
    pub const fn code(&self) -> u8 {
        self.retries()
    }
}

impl Default for MaxAckRetries {
    fn default() -> MaxAckRetries {
        MaxAckRetries(6)
    }
}
