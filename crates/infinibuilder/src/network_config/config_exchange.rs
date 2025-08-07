use crate::IbBTcpNetworkConfigExchangerError::{
    CommunicationError, ConnectionError, InvalidMessage, MessageTooLarge, RuntimeServerError,
};
use crate::{
    IbBCheckedStaticNetworkConfig, IbBReadyNetworkConfig, IbBReadyNodeConfig, IbBStaticNodeConfig,
};
use serde::de::Error;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};
use tokio::runtime::Runtime;
use tokio::sync::{Mutex, mpsc};

const MAX_MESSAGE_LENGTH: usize = 4096;

#[derive(Debug, Error)]
pub enum IbBTcpNetworkConfigExchangerError {
    #[error("Error during incoming TCP connection: {0}")]
    ConnectionError(std::io::Error),
    #[error("Error during TCP communication: {0}")]
    CommunicationError(std::io::Error),
    #[error("Invalid message: {0}")]
    InvalidMessage(serde_json::Error),
    #[error("Message is too large (length = {0}; max_length = {MAX_MESSAGE_LENGTH})")]
    MessageTooLarge(usize),
    #[error("Runtime server error: {0}")]
    RuntimeServerError(String),
}

pub struct IbBTcpNetworkConfigExchanger {
    runtime: Runtime,
}

pub struct IbBTcpNetworkConfigExchangerStream {
    stream: TcpStream,
}

impl IbBTcpNetworkConfigExchanger {
    pub fn new() -> Result<Self, IbBTcpNetworkConfigExchangerError> {
        let runtime = Runtime::new().map_err(|error| ConnectionError(error))?;
        Ok(Self { runtime })
    }

    pub fn await_receive_network_config(
        &self,
        socket_addr: impl ToSocketAddrs,
        network_config: &IbBCheckedStaticNetworkConfig,
        timeout: Duration,
    ) -> Result<IbBReadyNetworkConfig, IbBTcpNetworkConfigExchangerError> {
        self.runtime.block_on(async {
            self.run_network_config_server(socket_addr, network_config, timeout)
                .await
        })
    }

    async fn run_network_config_server(
        &self,
        socket_addr: impl ToSocketAddrs,
        network_config: &IbBCheckedStaticNetworkConfig,
        timeout: Duration,
    ) -> Result<IbBReadyNetworkConfig, IbBTcpNetworkConfigExchangerError> {
        let listener = TcpListener::bind(socket_addr)
            .await
            .map_err(|error| ConnectionError(error))?;

        let total_nodes = network_config.node_config_map.len();

        // Shared state for tracking received nodes by rank id
        let received_rank_ids = Arc::new(Mutex::new(HashSet::new()));
        let network_config = Arc::new(network_config.clone());

        // Channel for sending received ready nodes to main task
        let (tx, mut rx) = mpsc::channel::<
            Result<IbBReadyNodeConfig, IbBTcpNetworkConfigExchangerError>,
        >(total_nodes);

        // Spawn the connection acceptor task
        let listener_task = {
            let tx = tx.clone();
            let network_config = Arc::clone(&network_config);
            let received_rank_ids = Arc::clone(&received_rank_ids);

            tokio::spawn(async move {
                loop {
                    match listener.accept().await {
                        Ok((stream, peer_addr)) => {
                            println!("Received connection from: {}", peer_addr);

                            // Spawn a task to handle each connection in parallel
                            let tx_clone = tx.clone();
                            let network_config_clone = Arc::clone(&network_config);
                            let received_rank_ids_clone = Arc::clone(&received_rank_ids);

                            tokio::spawn(async move {
                                let result = Self::handle_connection(
                                    stream,
                                    &network_config_clone,
                                    &received_rank_ids_clone,
                                    timeout,
                                )
                                .await;

                                // Send result back to main task
                                if let Err(_) = tx_clone.send(result).await {
                                    println!(
                                        "Failed to send result for connection from {}",
                                        peer_addr
                                    );
                                }
                            });
                        }
                        Err(accept_error) => {
                            println!("Failed to accept connection: {}", accept_error);
                            let _ = tx.send(Err(ConnectionError(accept_error))).await;
                            break;
                        }
                    }
                }
            })
        };

        // Collect results from parallel connection handlers
        let mut successful_nodes = 0;
        let mut node_config_map = HashMap::new();
        let mut rank_ids = Vec::new();

        while successful_nodes < total_nodes {
            match rx.recv().await {
                Some(Ok(node_config)) => {
                    successful_nodes += 1;
                    rank_ids.push(node_config.rank_id());
                    node_config_map.insert(node_config.rank_id(), node_config);
                    println!(
                        "Successfully validated node ({}/{} nodes received)",
                        successful_nodes, total_nodes
                    );
                }
                Some(Err(error)) => {
                    println!("Connection validation failed: {}", error);
                    // Continue waiting for valid connections
                }
                None => {
                    return Err(RuntimeServerError(
                        "Channel closed unexpectedly".to_string(),
                    ));
                }
            }
        }

        // Cancel the listener task since we have all nodes
        listener_task.abort();

        println!("All {} nodes connected successfully!", total_nodes);

        // Create and return the ready network config
        Ok(IbBReadyNetworkConfig {
            node_config_map,
            rank_ids,
        })
    }

    async fn handle_connection(
        mut stream: TcpStream,
        network_config: &IbBCheckedStaticNetworkConfig,
        received_nodes: &Arc<Mutex<HashSet<u32>>>,
        timeout: Duration,
    ) -> Result<IbBReadyNodeConfig, IbBTcpNetworkConfigExchangerError> {
        let result = tokio::time::timeout(timeout, async {
            let mut buffer = [0u8; MAX_MESSAGE_LENGTH];
            let ready_node = Self::read_ready_node_from_stream(&mut stream, &mut buffer).await?;
            let validated_ready_node =
                Self::validate_ready_node_from_config(ready_node, network_config, received_nodes)
                    .await?;

            // Add the node to received_nodes to prevent duplicates
            received_nodes
                .lock()
                .await
                .insert(validated_ready_node.rank_id());

            // Return the ready node config
            Ok(validated_ready_node)
        })
        .await;

        result.unwrap_or_else(|_| {
            Err(CommunicationError(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Operation timed out",
            )))
        })
    }

    async fn validate_ready_node_from_config(
        ready_node: IbBReadyNodeConfig,
        network_config: &IbBCheckedStaticNetworkConfig,
        received_nodes: &Arc<Mutex<HashSet<u32>>>,
    ) -> Result<IbBReadyNodeConfig, IbBTcpNetworkConfigExchangerError> {
        let rank_id = ready_node.rank_id();

        if received_nodes.lock().await.contains(&rank_id) {
            return Err(InvalidMessage(serde_json::Error::custom(
                "Node already received",
            )));
        }

        match network_config.get(&rank_id) {
            Some(expected) if *expected == ready_node.node_config => Ok(ready_node),
            _ => Err(InvalidMessage(serde_json::Error::custom(
                "Node config mismatch",
            ))),
        }
    }

    async fn read_ready_node_from_stream(
        stream: &mut TcpStream,
        buffer: &mut [u8],
    ) -> Result<IbBReadyNodeConfig, IbBTcpNetworkConfigExchangerError> {
        // Read message size from 4 byte header
        let mut len_bytes = [0u8; 4];
        stream
            .read_exact(&mut len_bytes)
            .await
            .map_err(|error| CommunicationError(error))?;

        let len = u32::from_le_bytes(len_bytes) as usize;

        // Check size is within buffer boundaries
        if len > MAX_MESSAGE_LENGTH {
            return Err(MessageTooLarge(len));
        }

        // Read only the required bytes from the stream
        let msg_buffer = &mut buffer[..len];
        stream
            .read_exact(msg_buffer)
            .await
            .map_err(|error| CommunicationError(error))?;

        // Deserialize message payload
        let config = serde_json::from_slice(msg_buffer).map_err(|error| InvalidMessage(error))?;

        Ok(config)
    }
}
