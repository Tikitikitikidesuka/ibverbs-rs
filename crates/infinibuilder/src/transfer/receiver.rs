use crate::transfer::unsafe_slice::UnsafeSlice;
use derivative::Derivative;
use ibverbs::{
    CompletionQueue, MemoryRegion, PreparedQueuePair, ProtectionDomain, QueuePairEndpoint,
};
use serde::{Deserialize, Serialize};

pub struct ReceiverTransferConfig {
    num_senders: usize,
    memory_region: UnsafeSlice<u8>,
}

impl ReceiverTransferConfig {
    // Unsafe because it will unbind the memory region slice's lifetime
    pub unsafe fn new(num_senders: usize, memory_region: &[u8]) -> Self {
        Self {
            num_senders,
            memory_region: unsafe { UnsafeSlice::new(memory_region) },
        }
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct UnconnectedReceiverTransfer {
    #[derivative(Debug = "ignore")]
    prepared_qps: Vec<PreparedQueuePair>,
    qp_endpoints: Vec<QueuePairEndpoint>,
    #[derivative(Debug = "ignore")]
    mr: MemoryRegion<UnsafeSlice<u8>>,
    #[derivative(Debug = "ignore")]
    pd: ProtectionDomain,
    #[derivative(Debug = "ignore")]
    cq: CompletionQueue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiverConnectionOutputConfig {
    pub(crate) self_qp_endpoints: Vec<QueuePairEndpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiverConnectionInputConfig {
    pub(crate) remote_qp_endpoints: Vec<QueuePairEndpoint>,
}

impl ReceiverConnectionInputConfig {
    pub fn gather_connection_config(
        sender_configs: impl IntoIterator<Item = ReceiverConnectionOutputConfig>,
        receiver_idx: usize,
    ) {
        todo!()
    }
}

impl UnconnectedReceiverTransfer {}
