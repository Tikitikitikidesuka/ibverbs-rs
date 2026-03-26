use crate::network::config::{NetworkConfig, NodeConfig};
use ExchangeError::*;
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

/// An error that can occur during a TCP-based endpoint exchange.
#[derive(Debug, Error)]
pub enum ExchangeError {
    /// A rank referenced during the exchange is not present in the [`NetworkConfig`].
    #[error("Rank {rank} not in network")]
    InvalidRank { rank: usize },
    /// A message could not be serialized or deserialized.
    #[error("Error serializing/deserializing data ({0})")]
    SerdeError(#[from] serde_json::Error),
    /// An underlying TCP I/O operation failed.
    #[error("Error during IO operation ({0})")]
    IoError(#[from] std::io::Error),
    /// The serialized message exceeds `u32::MAX` bytes and cannot be framed
    /// with the 4-byte length prefix used by the wire protocol. The field
    /// contains the actual encoded size in bytes.
    #[error("Encoded message size {0} exceeds u32::MAX and cannot be framed")]
    MessageTooLarge(usize),
    /// The exchange did not complete within [`ExchangeConfig::exchange_timeout`].
    #[error("Exchange timed out")]
    Timeout,
}

/// Configuration for a TCP exchange operation.
pub struct ExchangeConfig {
    /// Maximum time to wait for the entire exchange to complete.
    pub exchange_timeout: Duration,
    /// Delay between connection retries when a remote peer is not yet listening.
    pub retry_delay: Duration,
}

impl Default for ExchangeConfig {
    fn default() -> Self {
        Self {
            exchange_timeout: Duration::from_secs(60),
            retry_delay: Duration::from_millis(1000),
        }
    }
}

/// TCP-based all-to-all data exchange between nodes in a network.
///
/// Used during setup to exchange RDMA endpoint information (e.g. [`QueuePairEndpoint`](crate::ibverbs::queue_pair::builder::QueuePairEndpoint))
/// between peers over TCP before RDMA channels are established.
pub struct Exchanger {}

#[derive(Debug, Deserialize, Serialize)]
struct ExchangeMessage<T> {
    rank: usize,
    data: T,
}

impl Exchanger {
    /// Exchanges `data` with all other nodes in the network, blocking until complete or timeout.
    ///
    /// Returns a `Vec<T>` indexed by rank, where the entry at this node's rank is a clone
    /// of the local `data` and all other entries come from the corresponding remote nodes.
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
            Self::exchange_all_connect(data, self_node, greater_ranks, network, config).await?;
        debug!("Done connecting");

        Ok(lower_nodes_data
            .into_iter()
            .chain(std::iter::once(data.to_owned()))
            .chain(greater_nodes_data)
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
            .map(|rank| {
                received
                    .remove(&rank)
                    .expect("rank should have been inserted by the exchange loop above")
            })
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
            .map(|rank| {
                received
                    .remove(&rank)
                    .expect("rank should have been inserted by the exchange loop above")
            })
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
        Ok(serde_json::from_slice(&msg_buf)?)
    }

    async fn write_stream<T: Serialize>(
        rank: usize,
        data: &T,
        stream: &mut (impl AsyncWriteExt + Unpin),
    ) -> Result<(), ExchangeError> {
        let encoded = serde_json::to_vec(&ExchangeMessage { rank, data })?;
        let len = u32::try_from(encoded.len()).map_err(|_| MessageTooLarge(encoded.len()))?;
        stream.write_all(len.to_be_bytes().as_ref()).await?;
        stream.write_all(encoded.as_slice()).await?;
        Ok(())
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn run_async<F: std::future::Future>(f: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(f)
    }

    #[test]
    fn write_read_round_trip_string() {
        run_async(async {
            let (mut writer, mut reader) = tokio::io::duplex(1024);
            Exchanger::write_stream(7, &"test data".to_string(), &mut writer)
                .await
                .unwrap();
            drop(writer);

            let msg: ExchangeMessage<String> =
                Exchanger::read_stream(&mut reader).await.unwrap();
            assert_eq!(msg.rank, 7);
            assert_eq!(msg.data, "test data");
        });
    }

    #[test]
    fn write_read_round_trip_struct() {
        #[derive(Debug, PartialEq, Serialize, Deserialize)]
        struct Endpoint {
            lid: u16,
            qpn: u32,
            psn: u32,
        }

        run_async(async {
            let endpoint = Endpoint {
                lid: 1,
                qpn: 0x1234,
                psn: 0xABCD,
            };

            let (mut writer, mut reader) = tokio::io::duplex(1024);
            Exchanger::write_stream(3, &endpoint, &mut writer)
                .await
                .unwrap();
            drop(writer);

            let msg: ExchangeMessage<Endpoint> =
                Exchanger::read_stream(&mut reader).await.unwrap();
            assert_eq!(msg.rank, 3);
            assert_eq!(msg.data, endpoint);
        });
    }

    #[test]
    fn write_read_round_trip_vec() {
        run_async(async {
            let data = vec![1u64, 2, 3, 4, 5];

            let (mut writer, mut reader) = tokio::io::duplex(1024);
            Exchanger::write_stream(0, &data, &mut writer)
                .await
                .unwrap();
            drop(writer);

            let msg: ExchangeMessage<Vec<u64>> =
                Exchanger::read_stream(&mut reader).await.unwrap();
            assert_eq!(msg.rank, 0);
            assert_eq!(msg.data, data);
        });
    }

    #[test]
    fn read_stream_rejects_truncated_length() {
        run_async(async {
            let data = [0u8, 1];
            let mut reader = &data[..];
            assert!(Exchanger::read_stream::<String>(&mut reader).await.is_err());
        });
    }

    #[test]
    fn read_stream_rejects_truncated_body() {
        run_async(async {
            let mut data = Vec::new();
            data.extend_from_slice(&100u32.to_be_bytes());
            data.extend_from_slice(&[0u8, 1]);
            let mut reader = &data[..];
            assert!(Exchanger::read_stream::<String>(&mut reader).await.is_err());
        });
    }

    #[test]
    fn insert_if_valid_accepts_valid_rank() {
        let mut received = HashMap::new();
        let msg = ExchangeMessage {
            rank: 2,
            data: "hello".to_string(),
        };
        assert!(Exchanger::insert_if_valid(msg, &mut received, 0..5));
        assert_eq!(received.get(&2).unwrap(), "hello");
    }

    #[test]
    fn insert_if_valid_rejects_out_of_range() {
        let mut received = HashMap::new();
        let msg = ExchangeMessage {
            rank: 10,
            data: "hello".to_string(),
        };
        assert!(!Exchanger::insert_if_valid(msg, &mut received, 0..5));
        assert!(received.is_empty());
    }

    #[test]
    fn insert_if_valid_overwrites_duplicate() {
        let mut received = HashMap::new();
        received.insert(2, "first".to_string());
        let msg = ExchangeMessage {
            rank: 2,
            data: "second".to_string(),
        };
        assert!(Exchanger::insert_if_valid(msg, &mut received, 0..5));
        assert_eq!(received.get(&2).unwrap(), "second");
    }

    fn make_network(ports: &[u16]) -> NetworkConfig {
        let mut builder = NetworkConfig::builder();
        for (i, &port) in ports.iter().enumerate() {
            builder = builder.add_node(
                NodeConfig::builder()
                    .hostname("127.0.0.1")
                    .port(port)
                    .ibdev("test0")
                    .rankid(i)
                    .build(),
            );
        }
        builder.build().unwrap()
    }

    #[test]
    fn two_node_exchange() {
        let network = make_network(&[41100, 41101]);

        let handles: Vec<_> = (0..2)
            .map(|rank| {
                let net = network.clone();
                std::thread::spawn(move || {
                    Exchanger::await_exchange_all(
                        rank,
                        &net,
                        &format!("from_{rank}"),
                        &ExchangeConfig::default(),
                    )
                })
            })
            .collect();

        let expected = vec!["from_0".to_string(), "from_1".to_string()];
        for handle in handles {
            assert_eq!(handle.join().unwrap().unwrap(), expected);
        }
    }

    #[test]
    fn three_node_exchange() {
        let network = make_network(&[41200, 41201, 41202]);

        let handles: Vec<_> = (0..3)
            .map(|rank| {
                let net = network.clone();
                std::thread::spawn(move || {
                    Exchanger::await_exchange_all(
                        rank,
                        &net,
                        &format!("from_{rank}"),
                        &ExchangeConfig::default(),
                    )
                })
            })
            .collect();

        let expected: Vec<String> = (0..3).map(|i| format!("from_{i}")).collect();
        for handle in handles {
            assert_eq!(handle.join().unwrap().unwrap(), expected);
        }
    }
}
