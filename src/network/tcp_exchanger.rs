use crate::network::config::{NetworkConfig, NodeConfig};
use ExchangeError::*;
use bincode::serde::{decode_from_slice, encode_to_vec};
use log::{debug, warn};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use std::ops::Range;
use std::time::Duration;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

#[derive(Debug, Error)]
pub enum ExchangeError {
    #[error("Rank {rank} not in network")]
    InvalidRank { rank: usize },
    #[error("Error decoding data ({0})")]
    DecodeError(#[from] bincode::error::DecodeError),
    #[error("Error encoding data ({0})")]
    EncodeError(#[from] bincode::error::EncodeError),
    #[error("Error during IO operation ({0})")]
    IoError(#[from] std::io::Error),
    #[error("Encoded message size {0} exceeds u32::MAX and cannot be framed")]
    MessageTooLarge(usize),
    #[error("")]
    Timeout,
}

pub struct ExchangeConfig {
    pub exchange_timeout: Duration, // Timeout for whole exchange
    pub retry_delay: Duration,      // Time during operation retries
}

impl Default for ExchangeConfig {
    fn default() -> Self {
        Self {
            exchange_timeout: Duration::from_secs(60),
            retry_delay: Duration::from_millis(1000),
        }
    }
}

pub struct Exchanger {}

#[derive(Debug, Deserialize, Serialize)]
struct ExchangeMessage<T> {
    rank: usize,
    data: T,
}

impl Exchanger {
    pub fn await_exchange_all<T: Serialize + DeserializeOwned + Clone>(
        rank: usize,
        network: &NetworkConfig,
        data: &T,
        config: &ExchangeConfig,
    ) -> Result<Vec<T>, ExchangeError> {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(async move {
                timeout(
                    config.exchange_timeout,
                    Self::exchange_all(rank, network, data, config),
                )
                .await
                .unwrap_or(Err(Timeout))
            })
    }

    // todo no longer needed?
    pub fn await_exchange_pair<T: Serialize + DeserializeOwned + Clone + Debug>(
        primary: bool,
        addr: (&str, u16),
        data: &T,
        config: &ExchangeConfig,
    ) -> Result<T, ExchangeError> {
        let rank = primary as usize;
        let peer = 1 - rank;

        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(async {
                timeout(config.exchange_timeout, async {
                    if primary {
                        let listener = TcpListener::bind(addr).await?;
                        let (mut stream, _) = listener.accept().await?;
                        let mut results = HashMap::new();
                        Self::exchange_serve(
                            data,
                            rank,
                            peer..(peer + 1),
                            &mut stream,
                            &mut results,
                        )
                        .await?;
                        Ok(results.into_values().next().expect("one inserted"))
                    } else {
                        let mut stream;
                        loop {
                            if let Ok(s) = TcpStream::connect(addr).await {
                                stream = s;
                                break;
                            }
                            tokio::time::sleep(config.retry_delay).await;
                        }
                        let mut results = HashMap::new();

                        Self::exchange_connect(
                            data,
                            rank,
                            peer..(peer + 1),
                            &mut stream,
                            &mut results,
                        )
                        .await?;
                        Ok(results.into_values().next().expect("one inserted"))
                    }
                })
                .await
                .unwrap_or(Err(Timeout))
            })
    }

    async fn exchange_all<T: Serialize + DeserializeOwned + Clone>(
        rank: usize,
        network: &NetworkConfig,
        data: &T,
        config: &ExchangeConfig,
    ) -> Result<Vec<T>, ExchangeError> {
        let self_node = network.get(rank).ok_or(InvalidRank { rank })?;
        let lower_ranks = 0..self_node.rankid;
        let greater_ranks = (self_node.rankid + 1)..(network.world_size());

        debug!(
            "Exchanging from {}:\n\tlower nodes -> {lower_ranks:?}\n\thigher nodes -> {greater_ranks:?}",
            self_node.rankid,
        );

        // Exchange server to lower nodes
        debug!("Serving exchange...");
        let lower_nodes_data = Self::exchange_all_serve(data, self_node, lower_ranks).await?;
        debug!("Done serving");

        // Exchange connect to greater nodes
        debug!("Connecting exchange...");
        let greater_nodes_data =
            Self::exchange_all_connect(data, self_node, greater_ranks, &network, &config).await?;
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
        remote_ranks: Range<usize>,
    ) -> Result<Vec<T>, ExchangeError> {
        let server = TcpListener::bind((self_node.hostname.as_str(), self_node.port)).await?;
        let mut received = HashMap::new();

        while received.len() < remote_ranks.len() {
            let (mut stream, _) = server.accept().await?;
            Self::exchange_serve(
                data,
                self_node.rankid,
                remote_ranks.clone(),
                &mut stream,
                &mut received,
            )
            .await?;
        }

        // Iterating on a map directly is O(capacity) so iterate with indices instead
        Ok(remote_ranks
            .map(|rank| received.remove(&rank).unwrap())
            .collect())
    }

    async fn exchange_all_connect<T: Serialize + DeserializeOwned>(
        data: &T,
        self_node: &NodeConfig,
        remote_ranks: Range<usize>,
        network: &NetworkConfig,
        config: &ExchangeConfig,
    ) -> Result<Vec<T>, ExchangeError> {
        let mut received = HashMap::new();

        for remote_rank in remote_ranks.clone() {
            let remote_node = network
                .get(remote_rank)
                .ok_or(InvalidRank { rank: remote_rank })?;

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
                remote_ranks.clone(),
                &mut stream,
                &mut received,
            )
            .await?;
        }

        // Iterating on a map directly is O(capacity) so iterate with indices instead
        Ok(remote_ranks
            .map(|rank| received.remove(&rank).unwrap())
            .collect())
    }

    async fn exchange_serve<T: Serialize + DeserializeOwned>(
        data: &T,
        self_rank: usize,
        remote_ranks: Range<usize>,
        stream: &mut TcpStream,
        received: &mut HashMap<usize, T>,
    ) -> Result<(), ExchangeError> {
        // Send self data
        Self::write_stream(self_rank, data, stream).await?;

        // Read incoming data
        let incoming_data = Self::read_stream::<T>(stream).await?;
        Self::insert_if_valid(incoming_data, received, remote_ranks.clone());

        Ok(())
    }

    async fn exchange_connect<T: Serialize + DeserializeOwned>(
        data: &T,
        self_rank: usize,
        remote_ranks: Range<usize>,
        stream: &mut TcpStream,
        received: &mut HashMap<usize, T>,
    ) -> Result<(), ExchangeError> {
        // Read incoming data
        let incoming_data = Self::read_stream::<T>(stream).await?;
        Self::insert_if_valid(incoming_data, received, remote_ranks.clone());

        // Send self data
        Self::write_stream(self_rank, data, stream).await?;

        Ok(())
    }

    fn insert_if_valid<T: Serialize + DeserializeOwned>(
        incoming_data: ExchangeMessage<T>,
        received: &mut HashMap<usize, T>,
        valid_range: Range<usize>,
    ) -> bool {
        // Validate rank is in range
        if valid_range.contains(&incoming_data.rank) {
            // Insert incoming data to map
            let out = received.insert(incoming_data.rank, incoming_data.data);
            if out.is_some() {
                // Warn if config already received for the specified rank id
                warn!("Duplicate exchange from {}", incoming_data.rank,);
            }
            debug!("Exchange progress -> {}", received.len());
            true
        } else {
            // Warn if exchange from invalid rank received
            warn!("Invalid rank incoming exchange {}", incoming_data.rank);
            false
        }
    }

    async fn read_stream<T: DeserializeOwned>(
        stream: &mut (impl AsyncReadExt + Unpin),
    ) -> Result<ExchangeMessage<T>, ExchangeError> {
        let mut size_buf = [0u8; size_of::<u32>()];
        stream.read_exact(&mut size_buf[..]).await?;
        let msg_size = u32::from_be_bytes(size_buf);

        let mut msg_buf = vec![0u8; msg_size as usize];
        stream.read_exact(&mut msg_buf[..]).await?;
        Ok(decode_from_slice(msg_buf.as_slice(), Self::bincode_config())?.0)
    }

    async fn write_stream<T: Serialize>(
        rank: usize,
        data: &T,
        stream: &mut (impl AsyncWriteExt + Unpin),
    ) -> Result<(), ExchangeError> {
        let encoded = encode_to_vec(ExchangeMessage { rank, data }, Self::bincode_config())?;
        let len = u32::try_from(encoded.len()).map_err(|_| MessageTooLarge(encoded.len()))?;
        stream
            .write_all(len.to_be_bytes().as_ref())
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
