use crate::network_config::{NetworkConfig, NodeConfig};
use TcpNetworkConfigExchangeError::*;
use bincode::serde::{decode_from_slice, encode_to_vec};
use log::{debug, warn};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::ops::Range;
use std::time::Duration;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

#[derive(Debug, Error)]
pub enum TcpNetworkConfigExchangeError {
    #[error("Rank id {rank_id} not in network")]
    InvalidRankId { rank_id: usize },
    #[error("Error decoding data ({0})")]
    DecodeError(#[from] bincode::error::DecodeError),
    #[error("Error encoding data ({0})")]
    EncodeError(#[from] bincode::error::EncodeError),
    #[error("Error during IO operation ({0})")]
    IoError(#[from] std::io::Error),
    #[error("")]
    Timeout,
}

pub struct TcpExchangeConfig {
    pub exchange_timeout: Duration, // Timeout for whole exchange
    pub retry_delay: Duration,      // Time during operation retries
}

impl Default for TcpExchangeConfig {
    fn default() -> Self {
        Self {
            exchange_timeout: Duration::from_secs(60),
            retry_delay: Duration::from_millis(1000),
        }
    }
}

pub struct TcpExchanger {}

#[derive(Debug, Serialize, Deserialize)]
struct ExchangeMessage<T> {
    rank_id: usize,
    data: T,
}

impl TcpExchanger {
    pub fn await_exchange_all<T: Serialize + DeserializeOwned + Clone>(
        rank_id: usize,
        network: &NetworkConfig,
        data: &T,
        config: &TcpExchangeConfig,
    ) -> Result<Vec<T>, TcpNetworkConfigExchangeError> {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(Self::exchange_all(rank_id, network, data, config))
    }

    pub async fn exchange_all<T: Serialize + DeserializeOwned + Clone>(
        rank_id: usize,
        network: &NetworkConfig,
        data: &T,
        config: &TcpExchangeConfig,
    ) -> Result<Vec<T>, TcpNetworkConfigExchangeError> {
        timeout(
            config.exchange_timeout,
            Self::exchange_all_inner(rank_id, network, data, config),
        )
        .await
        .unwrap_or(Err(Timeout))
    }

    async fn exchange_all_inner<T: Serialize + DeserializeOwned + Clone>(
        rank_id: usize,
        network: &NetworkConfig,
        data: &T,
        config: &TcpExchangeConfig,
    ) -> Result<Vec<T>, TcpNetworkConfigExchangeError> {
        let self_node = network.get(rank_id).ok_or(InvalidRankId { rank_id })?;
        let lower_rank_ids = network.rank_ids().start..self_node.rankid;
        let greater_rank_ids = (self_node.rankid + 1)..(network.rank_ids().end + 1);

        debug!(
            "Exchanging from {}:\n\tlower nodes -> {lower_rank_ids:?}\n\thigher nodes -> {greater_rank_ids:?}",
            self_node.rankid,
        );

        // Exchange server to lower nodes
        debug!("Serving exchange...");
        let lower_nodes_data =
            Self::exchange_all_serve(data, self_node, lower_rank_ids, &network, &config).await?;
        debug!("Done serving");

        // Exchange connect to greater nodes
        debug!("Connecting exchange...");
        let greater_nodes_data =
            Self::exchange_all_connect(data, self_node, greater_rank_ids, &network, &config)
                .await?;
        debug!("Done connecting");

        Ok(lower_nodes_data
            .into_iter()
            .chain(vec![data.to_owned()].into_iter())
            .chain(greater_nodes_data.into_iter())
            .collect())
    }

    async fn exchange_all_serve<T: Serialize + DeserializeOwned>(
        data: &T,
        self_node: &NodeConfig,
        remote_rank_ids: Range<usize>,
        network: &NetworkConfig,
        config: &TcpExchangeConfig,
    ) -> Result<Vec<T>, TcpNetworkConfigExchangeError> {
        let server = TcpListener::bind((self_node.hostname.as_str(), self_node.port)).await?;
        let mut received = HashMap::new();

        while received.len() < remote_rank_ids.len() {
            let (mut stream, _) = server.accept().await?;
            Self::exchange_serve(
                data,
                self_node.rankid,
                remote_rank_ids.clone(),
                &mut stream,
                &mut received,
            )
            .await?;
        }

        // Iterating on a map directly is O(capacity) so iterate with indices instead
        Ok(remote_rank_ids
            .map(|rank_id| received.remove(&rank_id).unwrap())
            .collect())
    }

    async fn exchange_all_connect<T: Serialize + DeserializeOwned>(
        data: &T,
        self_node: &NodeConfig,
        remote_rank_ids: Range<usize>,
        network: &NetworkConfig,
        config: &TcpExchangeConfig,
    ) -> Result<Vec<T>, TcpNetworkConfigExchangeError> {
        let mut received = HashMap::new();

        for remote_rank_id in remote_rank_ids.clone() {
            let remote_node = network.get(remote_rank_id).ok_or(InvalidRankId {
                rank_id: remote_rank_id,
            })?;

            let mut stream;
            loop {
                if let Ok(s) =
                    TcpStream::connect((remote_node.hostname.as_str(), remote_node.port)).await
                {
                    stream = s;
                    break;
                }
                tokio::time::sleep(config.retry_delay).await;
            }

            Self::exchange_connect(
                data,
                self_node.rankid,
                remote_rank_ids.clone(),
                &mut stream,
                &mut received,
            )
            .await?;
        }

        // Iterating on a map directly is O(capacity) so iterate with indices instead
        Ok(remote_rank_ids
            .map(|rank_id| received.remove(&rank_id).unwrap())
            .collect())
    }

    async fn exchange_serve<T: Serialize + DeserializeOwned>(
        data: &T,
        self_rank_id: usize,
        remote_rank_ids: Range<usize>,
        stream: &mut TcpStream,
        received: &mut HashMap<usize, T>,
    ) -> Result<(), TcpNetworkConfigExchangeError> {
        // Send self data
        Self::write_stream(self_rank_id, data, stream).await?;

        // Read incoming data
        let incoming_data = Self::read_stream::<T>(stream).await?;
        Self::insert_if_valid(incoming_data, received, remote_rank_ids.clone());

        Ok(())
    }

    async fn exchange_connect<T: Serialize + DeserializeOwned>(
        data: &T,
        self_rank_id: usize,
        remote_rank_ids: Range<usize>,
        stream: &mut TcpStream,
        received: &mut HashMap<usize, T>,
    ) -> Result<(), TcpNetworkConfigExchangeError> {
        // Read incoming data
        let incoming_data = Self::read_stream::<T>(stream).await?;
        Self::insert_if_valid(incoming_data, received, remote_rank_ids.clone());

        // Send self data
        Self::write_stream(self_rank_id, data, stream).await?;

        Ok(())
    }

    fn insert_if_valid<T: Serialize + DeserializeOwned>(
        incoming_data: ExchangeMessage<T>,
        received: &mut HashMap<usize, T>,
        valid_range: Range<usize>,
    ) -> bool {
        // Validate rank id is in range
        if valid_range.contains(&incoming_data.rank_id) {
            // Insert incoming data to map
            let out = received.insert(incoming_data.rank_id, incoming_data.data);
            if out.is_some() {
                // Warn if config already received for the specified rank id
                warn!("Duplicate exchange from {}", incoming_data.rank_id,);
            }
            debug!("Exchange progress -> {}", received.len());
            true
        } else {
            // Warn if exchange from invalid rank id received
            warn!(
                "Invalid rank id incoming exchange {}",
                incoming_data.rank_id
            );
            false
        }
    }

    async fn read_stream<T: DeserializeOwned>(
        stream: &mut (impl AsyncReadExt + Unpin),
    ) -> Result<ExchangeMessage<T>, TcpNetworkConfigExchangeError> {
        let mut size_buf = [0u8; size_of::<u32>()];
        stream.read_exact(&mut size_buf[..]).await?;
        let msg_size = u32::from_be_bytes(size_buf);

        let mut msg_buf = vec![0u8; msg_size as usize];
        stream.read_exact(&mut msg_buf[..]).await?;
        Ok(decode_from_slice(msg_buf.as_slice(), Self::bincode_config())?.0)
    }

    async fn write_stream<T: Serialize>(
        rank_id: usize,
        data: &T,
        stream: &mut (impl AsyncWriteExt + Unpin),
    ) -> Result<(), TcpNetworkConfigExchangeError> {
        let encoded = encode_to_vec(ExchangeMessage { rank_id, data }, Self::bincode_config())?;
        stream
            .write_all((encoded.len() as u32).to_be_bytes().as_ref())
            .await?;
        stream.write_all(encoded.as_slice()).await?;
        Ok(())
    }

    fn bincode_config() -> impl bincode::config::Config {
        bincode::config::standard()
            .with_big_endian()
            .with_variable_int_encoding()
    }
}
