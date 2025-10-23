use thiserror::Error;

#[derive(Debug, Copy, Clone, Error)]
#[error("Ibverbs work error: {code} (vendor_code={vendor_code})")]
pub struct IbvWorkError {
    code: IbvWorkErrorCode,
    vendor_code: u32,
}

impl IbvWorkError {
    pub(super) fn new(code: IbvWorkErrorCode, vendor_code: u32) -> Self {
        Self { code, vendor_code }
    }
}

impl IbvWorkError {
    pub fn code(&self) -> IbvWorkErrorCode {
        self.code
    }

    pub fn vendor_code(&self) -> u32 {
        self.vendor_code
    }
}

#[derive(Debug, Copy, Clone, Error)]
pub enum IbvWorkErrorCode {
    #[error("Local length error")]
    LocalLengthError,

    #[error("Local queue pair operation error")]
    LocalQueuePairOperationError,

    #[error("Local EEC operation error")]
    LocalEecOperationError,

    #[error("Local protection error")]
    LocalProtectionError,

    #[error("Work request flush error")]
    WorkRequestFlushError,

    #[error("Memory window bind error")]
    MemoryWindowBindError,

    #[error("Bad response error")]
    BadResponseError,

    #[error("Local access error")]
    LocalAccessError,

    #[error("Remote invalid request error")]
    RemoteInvalidRequestError,

    #[error("Remote access error")]
    RemoteAccessError,

    #[error("Remote operation error")]
    RemoteOperationError,

    #[error("Retry exceeded error")]
    RetryExceededError,

    #[error("RNR retry exceeded error")]
    RnrRetryExceededError,

    #[error("Local RDD violation error")]
    LocalRddViolationError,

    #[error("Remote invalid RD request error")]
    RemoteInvalidReadRequestError,

    #[error("Remote abort error")]
    RemoteAbortError,

    #[error("Invalid EECN error")]
    InvalidEecnError,

    #[error("Invalid EEC state error")]
    InvalidEecStateError,

    #[error("Fatal error")]
    FatalError,

    #[error("Response timeout error")]
    ResponseTimeoutError,

    #[error("General error")]
    GeneralError,

    #[error("Tag matching error")]
    TagMatchingError,

    #[error("Tag matching rendezvous incomplete")]
    TagMatchingRendezvousIncomplete,

    #[error("Unknown error")]
    UnknownError,
}

impl TryFrom<u32> for IbvWorkErrorCode {
    type Error = ();

    /// Only fails if it was actually a success status
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Err(()),
            1 => Ok(Self::LocalLengthError),
            2 => Ok(Self::LocalQueuePairOperationError),
            3 => Ok(Self::LocalEecOperationError),
            4 => Ok(Self::LocalProtectionError),
            5 => Ok(Self::WorkRequestFlushError),
            6 => Ok(Self::MemoryWindowBindError),
            7 => Ok(Self::BadResponseError),
            8 => Ok(Self::LocalAccessError),
            9 => Ok(Self::RemoteInvalidRequestError),
            10 => Ok(Self::RemoteAccessError),
            11 => Ok(Self::RemoteOperationError),
            12 => Ok(Self::RetryExceededError),
            13 => Ok(Self::RnrRetryExceededError),
            14 => Ok(Self::LocalRddViolationError),
            15 => Ok(Self::RemoteInvalidReadRequestError),
            16 => Ok(Self::RemoteAbortError),
            17 => Ok(Self::InvalidEecnError),
            18 => Ok(Self::InvalidEecStateError),
            19 => Ok(Self::FatalError),
            20 => Ok(Self::ResponseTimeoutError),
            21 => Ok(Self::GeneralError),
            22 => Ok(Self::TagMatchingError),
            23 => Ok(Self::TagMatchingRendezvousIncomplete),
            _ => Ok(Self::UnknownError),
        }
    }
}
