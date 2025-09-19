use crate::rdma_traits::{WorkCompletion, WorkCompletionSuccess};
use ibverbs::{ibv_wc, ibv_wc_status};

impl From<ibv_wc> for WorkCompletion {
    fn from(wc: ibv_wc) -> Self {
        match (wc.is_valid(), wc.error()) {
            (false, None) => Self::Error(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Unknown error".to_string(),
            )),
            (true, _) => Self::Success(WorkCompletionSuccess {
                len: wc.len(),
                imm_data: wc.imm_data(),
            }),
            (_, Some((ibv_wc_status::IBV_WC_LOC_LEN_ERR, v))) => Self::Error(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Local length error (vendor_code={})", v),
            )),
            (_, Some((ibv_wc_status::IBV_WC_LOC_QP_OP_ERR, v))) => Self::Error(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Local QP operation error (vendor_code={})", v),
            )),
            (_, Some((ibv_wc_status::IBV_WC_LOC_EEC_OP_ERR, v))) => Self::Error(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Local EEC operation error (vendor_code={})", v),
            )),
            (_, Some((ibv_wc_status::IBV_WC_LOC_PROT_ERR, v))) => Self::Error(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("Local protection error (vendor_code={})", v),
            )),
            (_, Some((ibv_wc_status::IBV_WC_WR_FLUSH_ERR, v))) => Self::Error(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                format!("Work Request flushed error (vendor_code={})", v),
            )),
            (_, Some((ibv_wc_status::IBV_WC_MW_BIND_ERR, v))) => Self::Error(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Memory Window bind error (vendor_code={})", v),
            )),
            (_, Some((ibv_wc_status::IBV_WC_BAD_RESP_ERR, v))) => Self::Error(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Bad response error (vendor_code={})", v),
            )),
            (_, Some((ibv_wc_status::IBV_WC_LOC_ACCESS_ERR, v))) => Self::Error(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("Local access error (vendor_code={})", v),
            )),
            (_, Some((ibv_wc_status::IBV_WC_REM_INV_REQ_ERR, v))) => Self::Error(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Remote invalid request error (vendor_code={})", v),
            )),
            (_, Some((ibv_wc_status::IBV_WC_REM_ACCESS_ERR, v))) => Self::Error(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("Remote access error (vendor_code={})", v),
            )),
            (_, Some((ibv_wc_status::IBV_WC_REM_OP_ERR, v))) => Self::Error(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Remote operation error (vendor_code={})", v),
            )),
            (_, Some((ibv_wc_status::IBV_WC_RETRY_EXC_ERR, v))) => Self::Error(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("Retry exceeded error (vendor_code={})", v),
            )),
            (_, Some((ibv_wc_status::IBV_WC_RNR_RETRY_EXC_ERR, v))) => Self::Error(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("Receiver not ready retry exceeded (vendor_code={})", v),
            )),
            (_, Some((ibv_wc_status::IBV_WC_LOC_RDD_VIOL_ERR, v))) => Self::Error(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Local RDD violation (vendor_code={})", v),
            )),
            (_, Some((ibv_wc_status::IBV_WC_REM_INV_RD_REQ_ERR, v))) => Self::Error(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Remote invalid RD request (vendor_code={})", v),
            )),
            (_, Some((ibv_wc_status::IBV_WC_REM_ABORT_ERR, v))) => Self::Error(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                format!("Remote aborted (vendor_code={})", v),
            )),
            (_, Some((ibv_wc_status::IBV_WC_INV_EECN_ERR, v))) => Self::Error(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Invalid EECN error (vendor_code={})", v),
            )),
            (_, Some((ibv_wc_status::IBV_WC_INV_EEC_STATE_ERR, v))) => Self::Error(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Invalid EEC state error (vendor_code={})", v),
            )),
            (_, Some((ibv_wc_status::IBV_WC_FATAL_ERR, v))) => Self::Error(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Fatal error (vendor_code={})", v),
            )),
            (_, Some((ibv_wc_status::IBV_WC_RESP_TIMEOUT_ERR, v))) => Self::Error(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("Response timeout (vendor_code={})", v),
            )),
            (_, Some((ibv_wc_status::IBV_WC_GENERAL_ERR, v))) => Self::Error(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("General error (vendor_code={})", v),
            )),
            (_, Some((ibv_wc_status::IBV_WC_TM_ERR, v))) => Self::Error(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Transport manager error (vendor_code={})", v),
            )),
            (_, Some((ibv_wc_status::IBV_WC_TM_RNDV_INCOMPLETE, v))) => Self::Error(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!(
                    "Transport manager rendezvous incomplete (vendor_code={})",
                    v
                ),
            )),
            _ => Self::Error(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Unhandled error".to_string(),
            )),
        }
    }
}
