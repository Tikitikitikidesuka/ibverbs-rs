use ibverbs_sys::ibv_wc_status;
use num_enum::FromPrimitive;
use std::{fmt, io};
use thiserror::Error;

#[derive(Copy, Clone, Debug, Error)]
pub struct WorkError {
    raw_status: u32,
    vendor_code: u32,
}

impl WorkError {
    /// The raw status cannot be IBV_WC_SUCCESS.
    pub(super) fn new(raw_status: ibv_wc_status::Type, vendor_code: u32) -> Self {
        Self {
            raw_status,
            vendor_code,
        }
    }

    /// Raw `ibv_wc.status` value.
    pub fn raw_status(&self) -> u32 {
        self.raw_status
    }

    /// Vendor-specific error code.
    pub fn vendor_code(&self) -> u32 {
        self.vendor_code
    }

    /// Canonical ibverbs error code derived from `raw_status`.
    pub fn code(&self) -> WorkErrorCode {
        WorkErrorCode::from(self.raw_status)
    }
}

impl fmt::Display for WorkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = self.code();

        write!(
            f,
            "{} [{:?}] \
             (raw_status={}, vendor_code={}, hint={})",
            code,
            code.class(),
            self.raw_status,
            self.vendor_code,
            code.hint(),
        )
    }
}

/// Broad classification of where the failure occurred.
#[derive(Debug, Copy, Clone)]
pub enum WorkErrorClass {
    /// Bug or invalid usage in local application code.
    LocalProgrammingError,

    /// Local resource exhaustion or QP teardown.
    LocalResourceError,

    /// Error reported by the remote peer.
    RemoteError,

    /// Transport-level retry or link failure.
    TransportError,

    /// Timeout waiting for a response.
    Timeout,

    /// Fatal device or QP error.
    Fatal,

    /// Uncategorized or unknown failure.
    Unknown,
}

/// Canonical ibverbs WC error codes.
///
/// Numeric values match `enum ibv_wc_status`.
#[derive(Debug, Copy, Clone, Error, FromPrimitive)]
#[repr(u32)]
pub enum WorkErrorCode {
    #[error("local length error")]
    LocalLengthError = 1,

    #[error("local queue pair operation error")]
    LocalQueuePairOperationError = 2,

    #[error("local EEC operation error")]
    LocalEecOperationError = 3,

    #[error("local protection error")]
    LocalProtectionError = 4,

    #[error("work request flush error")]
    WorkRequestFlushError = 5,

    #[error("memory window bind error")]
    MemoryWindowBindError = 6,

    #[error("bad response error")]
    BadResponseError = 7,

    #[error("local access error")]
    LocalAccessError = 8,

    #[error("remote invalid request error")]
    RemoteInvalidRequestError = 9,

    #[error("remote access error")]
    RemoteAccessError = 10,

    #[error("remote operation error")]
    RemoteOperationError = 11,

    #[error("retry exceeded error")]
    RetryExceededError = 12,

    #[error("RNR retry exceeded error")]
    RnrRetryExceededError = 13,

    #[error("local RDD violation error")]
    LocalRddViolationError = 14,

    #[error("remote invalid RD request error")]
    RemoteInvalidReadRequestError = 15,

    #[error("remote abort error")]
    RemoteAbortError = 16,

    #[error("invalid EECN error")]
    InvalidEecnError = 17,

    #[error("invalid EEC state error")]
    InvalidEecStateError = 18,

    #[error("fatal error")]
    FatalError = 19,

    #[error("response timeout error")]
    ResponseTimeoutError = 20,

    #[error("general error")]
    GeneralError = 21,

    #[error("tag matching error")]
    TagMatchingError = 22,

    #[error("tag matching rendezvous incomplete")]
    TagMatchingRendezvousIncomplete = 23,

    #[error("unknown error")]
    #[num_enum(default)]
    UnknownError,
}

impl WorkErrorCode {
    /// Classify the failure domain.
    pub fn class(self) -> WorkErrorClass {
        use WorkErrorClass::*;
        use WorkErrorCode::*;

        match self {
            LocalLengthError
            | LocalProtectionError
            | LocalAccessError
            | LocalQueuePairOperationError
            | LocalEecOperationError
            | InvalidEecnError
            | InvalidEecStateError
            | LocalRddViolationError => LocalProgrammingError,

            WorkRequestFlushError | MemoryWindowBindError => LocalResourceError,

            RemoteInvalidRequestError
            | RemoteInvalidReadRequestError
            | RemoteAccessError
            | RemoteOperationError
            | RemoteAbortError => RemoteError,

            RetryExceededError | RnrRetryExceededError => TransportError,

            ResponseTimeoutError => Timeout,

            FatalError => Fatal,

            GeneralError
            | TagMatchingError
            | TagMatchingRendezvousIncomplete
            | BadResponseError
            | UnknownError => Unknown,
        }
    }

    /// Practical debugging hint.
    pub fn hint(self) -> &'static str {
        use WorkErrorCode::*;

        match self {
            LocalLengthError => "SGE length exceeds MR bounds or WR length is invalid",

            LocalProtectionError => {
                "Memory region permissions do not allow this operation \
                 (check LOCAL_WRITE / REMOTE_READ / REMOTE_WRITE flags)"
            }

            LocalAccessError => "DMA failed due to invalid or unmapped memory",

            LocalQueuePairOperationError => {
                "Work request posted in invalid QP state or illegal opcode"
            }

            WorkRequestFlushError => "QP entered error state; outstanding WRs were flushed",

            MemoryWindowBindError => "Memory window bind failed (invalid MR or access flags)",

            RemoteInvalidRequestError => {
                "Remote QP rejected the request (bad rkey, addr, or opcode)"
            }

            RemoteInvalidReadRequestError => {
                "Remote rejected RDMA read (address or length invalid)"
            }

            RemoteAccessError => "Remote memory protection violation (check rkey and permissions)",

            RemoteOperationError => "Remote QP failed processing the request",

            RetryExceededError => {
                "Packet retry limit exceeded (link issue or remote QP unresponsive)"
            }

            RnrRetryExceededError => {
                "Receiver Not Ready retry limit exceeded (remote CQ/WQ stalled)"
            }

            ResponseTimeoutError => "No response before timeout (QP stalled or fabric issue)",

            FatalError => "Fatal QP or device error; QP is no longer usable",

            _ => "No additional diagnostic information available",
        }
    }
}
