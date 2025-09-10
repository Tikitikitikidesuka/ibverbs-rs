use crate::transfer::common::{
    ConnectedTransfer, ConnectionConfigGatherError, ConnectionInputConfig, ConnectionOutputConfig,
    TransferConfig, TransferError, UnconnectedTransfer,
};
use crate::transfer::receiver::ReceiverConnectionOutputConfig;
use crate::transfer::request::TransferRequest;
use ibverbs::ibv_wc;
use serde::{Deserialize, Serialize};
use std::ops::RangeBounds;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SenderConnectionOutputConfig {
    pub inner: ConnectionOutputConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SenderConnectionInputConfig {
    pub inner: ConnectionInputConfig,
}

impl SenderConnectionInputConfig {
    pub fn gather_from_receivers(
        receiver_configs: impl IntoIterator<Item = ReceiverConnectionOutputConfig>,
        sender_idx: usize,
    ) -> Result<Self, ConnectionConfigGatherError> {
        ConnectionInputConfig::gather_connection_config(
            receiver_configs
                .into_iter()
                .map(|receiver_config| receiver_config.inner),
            sender_idx,
        )
        .map(|input_config| Self {
            inner: input_config,
        })
    }
}

pub struct UnconnectedSenderTransfer {
    inner: UnconnectedTransfer,
}

impl UnconnectedSenderTransfer {
    pub fn new(ib_context: &ibverbs::Context, config: TransferConfig) -> std::io::Result<Self> {
        Ok(Self {
            inner: UnconnectedTransfer::new(ib_context, config)?,
        })
    }

    pub fn connection_config(&self) -> SenderConnectionOutputConfig {
        SenderConnectionOutputConfig {
            inner: self.inner.connection_config(),
        }
    }

    pub fn connect(
        self,
        connection_config: SenderConnectionInputConfig,
    ) -> std::io::Result<SenderTransfer> {
        Ok(SenderTransfer {
            inner: self.inner.connect(connection_config.inner)?,
        })
    }
}

pub struct SenderTransfer {
    inner: ConnectedTransfer,
}

impl SenderTransfer {
    pub fn post_send(
        &mut self,
        receiver_idx: usize,
        memory_range: impl RangeBounds<usize>,
    ) -> Result<TransferRequest, TransferError> {
        self.inner.post_send(receiver_idx, memory_range)
    }

    pub fn wait_send(
        &mut self,
        receiver_idx: usize,
        memory_range: impl RangeBounds<usize>,
    ) -> Result<ibv_wc, TransferError> {
        self.inner.wait_send(receiver_idx, memory_range)
    }
}
