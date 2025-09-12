use crate::component::UnconnectedComponent;
use crate::synchronization::centralized::common::CentralizedSyncConnectionInputConfig;
use crate::transfer::common::{
    ConnectionConfigGatherError, ConnectionInputConfig, ConnectionOutputConfig, Transfer,
    TransferConfig, TransferError, UnconnectedTransfer,
};
use crate::transfer::request::TransferRequest;
use crate::transfer::sender::SenderConnectionOutputConfig;
use ibverbs::ibv_wc;
use serde::{Deserialize, Serialize};
use std::ops::RangeBounds;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiverConnectionOutputConfig {
    pub inner: ConnectionOutputConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiverConnectionInputConfig {
    pub inner: ConnectionInputConfig,
}

impl ReceiverConnectionInputConfig {
    pub fn gather_from_senders(
        sender_configs: impl IntoIterator<Item = SenderConnectionOutputConfig>,
        receiver_idx: usize,
    ) -> Result<Self, ConnectionConfigGatherError> {
        ConnectionInputConfig::gather_connection_config(
            sender_configs
                .into_iter()
                .map(|sender_config| sender_config.inner),
            receiver_idx,
        )
        .map(|input_config| Self {
            inner: input_config,
        })
    }
}

pub struct UnconnectedReceiverTransfer {
    inner: UnconnectedTransfer,
}

impl UnconnectedReceiverTransfer {
    pub fn new(ib_context: &ibverbs::Context, config: TransferConfig) -> std::io::Result<Self> {
        Ok(Self {
            inner: UnconnectedTransfer::new(ib_context, config)?,
        })
    }
}

impl UnconnectedComponent for UnconnectedReceiverTransfer {
    type ConnectionOutputConfig = ReceiverConnectionOutputConfig;
    type ConnectionInputConfig = ReceiverConnectionInputConfig;
    type ConnectedComponent = ReceiverTransfer;

    fn connection_config(&self) -> ReceiverConnectionOutputConfig {
        ReceiverConnectionOutputConfig {
            inner: self.inner.connection_config(),
        }
    }

    fn connect(
        self,
        connection_config: ReceiverConnectionInputConfig,
    ) -> std::io::Result<ReceiverTransfer> {
        Ok(ReceiverTransfer {
            inner: self.inner.connect(connection_config.inner)?,
        })
    }
}

pub struct ReceiverTransfer {
    inner: Transfer,
}

impl ReceiverTransfer {
    pub fn post_receive(
        &mut self,
        sender_idx: usize,
        memory_range: impl RangeBounds<usize>,
    ) -> Result<TransferRequest, TransferError> {
        self.inner.post_receive(sender_idx, memory_range)
    }

    pub fn wait_receive(
        &mut self,
        sender_idx: usize,
        memory_range: impl RangeBounds<usize>,
    ) -> Result<ibv_wc, TransferError> {
        self.inner.wait_receive(sender_idx, memory_range)
    }
}
