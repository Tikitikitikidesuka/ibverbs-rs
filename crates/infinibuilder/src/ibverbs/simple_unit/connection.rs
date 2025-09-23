use crate::connect::Connect;
use crate::ibverbs::cached_cq::CachedCompletionQueue;
use derivative::Derivative;
use ibverbs::{
    CompletionQueue, Context, PreparedQueuePair, ProtectionDomain, QueuePair, QueuePairEndpoint,
    ibv_access_flags, ibv_qp_type,
};
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use serde::{Deserialize, Serialize};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct UnconnectedIbvConnection {
    #[derivative(Debug = "ignore")]
    prepared_qp: PreparedQueuePair,
    qp_endpoint: QueuePairEndpoint,
    #[derivative(Debug = "ignore")]
    pub(super) pd: ProtectionDomain,
    #[derivative(Debug = "ignore")]
    pub(super) cq: CompletionQueue,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IbvConnectionConfig {
    qp_endpoint: QueuePairEndpoint,
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct IbvConnection {
    local_qp_endpoint: QueuePairEndpoint,
    remote_qp_endpoint: QueuePairEndpoint,
    #[derivative(Debug = "ignore")]
    pub(super) qp: QueuePair,
    #[derivative(Debug = "ignore")]
    pub(super) _pd: ProtectionDomain,
    #[derivative(Debug = "ignore")]
    pub(super) cached_cq: Arc<CachedCompletionQueue>,
}

impl UnconnectedIbvConnection {
    pub fn new<const CQ_SIZE: usize>(ib_context: &Context) -> std::io::Result<Self> {
        let cq = ib_context.create_cq(CQ_SIZE as i32, 0)?;
        let pd = ib_context.alloc_pd()?;
        let prepared_qp = pd
            .create_qp(&cq, &cq, ibv_qp_type::IBV_QPT_RC)?
            .set_access(
                ibv_access_flags::IBV_ACCESS_REMOTE_WRITE
                    | ibv_access_flags::IBV_ACCESS_REMOTE_READ
                    | ibv_access_flags::IBV_ACCESS_LOCAL_WRITE,
            )
            .build()?;
        let qp_endpoint = prepared_qp.endpoint()?;

        Ok(Self {
            prepared_qp,
            qp_endpoint,
            pd,
            cq,
        })
    }
}

impl Connect for UnconnectedIbvConnection {
    type ConnectionConfig = IbvConnectionConfig;
    type Connected = IbvConnection;

    fn connection_config(&self) -> Self::ConnectionConfig {
        IbvConnectionConfig {
            qp_endpoint: self.qp_endpoint,
        }
    }

    fn connect(self, connection_config: IbvConnectionConfig) -> std::io::Result<IbvConnection> {
        Ok(IbvConnection {
            local_qp_endpoint: self.qp_endpoint,
            remote_qp_endpoint: connection_config.qp_endpoint,
            qp: self.prepared_qp.handshake(connection_config.qp_endpoint)?,
            _pd: self.pd,
            cached_cq: Arc::new(CachedCompletionQueue::new(self.cq)),
        })
    }
}