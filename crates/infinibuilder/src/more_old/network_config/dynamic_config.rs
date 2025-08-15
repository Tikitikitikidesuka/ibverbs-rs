use ibverbs::QueuePairEndpoint;
use serde::{Deserialize, Serialize};

// Configuration that is generated on runtime
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IbBDynamicNodeConfig {
    pub qp_endpoint: QueuePairEndpoint,
}