use crate::IbBTcpNetworkConfigExchangerError::{
    CommunicationError, ConnectionError, InvalidMessage, MessageTooLarge, NonExistentRankId,
    RuntimeServerError,
};
use crate::network_config::dynamic_config::IbBDynamicNodeConfig;
use crate::{
    IbBCheckedStaticNetworkConfig, IbBReadyNetworkConfig, IbBReadyNodeConfig, IbBStaticNodeConfig,
};
use futures::future::join_all;
use futures::join;
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
    #[error("Rank id is not in network config {0}")]
    NonExistentRankId(u32),
}

pub struct IbBTcpNetworkConfigExchanger;

pub struct IbBTcpNetworkConfigExchangerConfig {
    pub tcp_port: u16,                        // Port for exchange over tcp
    pub send_timeout: Duration,               // Timeout for whole network send
    pub send_attempt_delay: Duration,         // Delay between send attempts
    pub receive_timeout: Duration,            // Timeout for whole network receive
    pub receive_connection_timeout: Duration, // Timeout per connection in receive
}

impl IbBTcpNetworkConfigExchanger {
    pub fn await_exchange_network_config(
        self_rank_id: u32,
        self_dynamic_config: &IbBDynamicNodeConfig,
        socket_addr: impl ToSocketAddrs,
        network_config: &IbBCheckedStaticNetworkConfig,
        exchanger_config: &IbBTcpNetworkConfigExchangerConfig,
    ) -> Result<IbBReadyNetworkConfig, IbBTcpNetworkConfigExchangerError> {
        Runtime::new()
            .map_err(|error| ConnectionError(error))?
            .block_on(Self::exchange_network_config(
                self_rank_id,
                self_dynamic_config,
                socket_addr,
                network_config,
                exchanger_config,
            ))
    }

    pub async fn exchange_network_config(
        self_rank_id: u32,
        self_dynamic_config: &IbBDynamicNodeConfig,
        socket_addr: impl ToSocketAddrs,
        network_config: &IbBCheckedStaticNetworkConfig,
        exchanger_config: &IbBTcpNetworkConfigExchangerConfig,
    ) -> Result<IbBReadyNetworkConfig, IbBTcpNetworkConfigExchangerError> {
        let send_fut = Self::send_network_config(
            self_rank_id,
            self_dynamic_config,
            &network_config,
            &exchanger_config,
        );

        let recv_fut =
            Self::receive_network_config(socket_addr, &network_config, &exchanger_config);

        let (send_result, recv_result) = join!(send_fut, recv_fut);

        // Prioritize returning receive errors if they occur
        match (send_result, recv_result) {
            (Ok(_), Ok(ready_config)) => Ok(ready_config),
            (Err(e), _) => Err(e),
            (_, Err(e)) => Err(e),
        }
    }

    pub async fn send_network_config(
        self_rank_id: u32,
        self_dynamic_config: &IbBDynamicNodeConfig,
        network_config: &IbBCheckedStaticNetworkConfig,
        exchanger_config: &IbBTcpNetworkConfigExchangerConfig,
    ) -> Result<(), IbBTcpNetworkConfigExchangerError> {
        tokio::time::timeout(
            exchanger_config.send_timeout,
            Self::send_config_to_nodes_async(
                self_rank_id,
                self_dynamic_config,
                network_config,
                exchanger_config,
            ),
        )
        .await
        .unwrap_or_else(|_| {
            Err(CommunicationError(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Operation timed out",
            )))
        })
    }

    // Will loop retrying failed sends until all are finished successfully
    // Must be run with a timeout block to prevent infinite loop
    async fn send_config_to_nodes_async(
        self_rank_id: u32,
        self_dynamic_config: &IbBDynamicNodeConfig,
        network_config: &IbBCheckedStaticNetworkConfig,
        exchanger_config: &IbBTcpNetworkConfigExchangerConfig,
    ) -> Result<(), IbBTcpNetworkConfigExchangerError> {
        let self_node_config = network_config
            .node_config_map
            .get(&self_rank_id)
            .ok_or(NonExistentRankId(self_rank_id))?
            .clone();

        let self_ready_node_config = IbBReadyNodeConfig {
            node_config: self_node_config,
            dynamic_config: self_dynamic_config.clone(),
        };

        // Serialize the ready node config once
        let message_payload =
            serde_json::to_vec(&self_ready_node_config).map_err(|error| InvalidMessage(error))?;

        if message_payload.len() > MAX_MESSAGE_LENGTH {
            return Err(MessageTooLarge(message_payload.len()));
        }

        // Create tasks for sending to all nodes concurrently
        let mut send_tasks = Vec::new();

        for node_config in network_config.iter() {
            let rank_id = node_config.rank_id();
            let hostname = node_config.hostname().to_string();
            let tcp_port = exchanger_config.tcp_port;
            let attempt_delay = exchanger_config.send_attempt_delay;
            let message_payload = message_payload.clone();

            let task = tokio::spawn(async move {
                Self::loop_attempt_send_to_single_node(
                    &hostname,
                    tcp_port,
                    &message_payload,
                    attempt_delay,
                    rank_id,
                )
                .await
            });

            send_tasks.push(task);
        }

        // Wait for all send operations to complete concurrently
        // Since loop_attempt_send_to_single_node never returns Err, only check for join errors
        let results = join_all(send_tasks).await;

        for result in results {
            if let Err(join_error) = result {
                return Err(RuntimeServerError(format!(
                    "Task join error: {}",
                    join_error
                )));
            }
        }

        Ok(())
    }

    async fn loop_attempt_send_to_single_node(
        hostname: &str,
        tcp_port: u16,
        message_payload: &[u8],
        attempt_delay: Duration,
        rank_id: u32,
    ) -> Result<(), IbBTcpNetworkConfigExchangerError> {
        let target_addr = format!("{}:{}", hostname, tcp_port);
        let mut attempt = 1;

        loop {
            match Self::attempt_send(&target_addr, message_payload).await {
                Ok(()) => {
                    println!(
                        "Successfully sent config to node {} ({})",
                        rank_id, target_addr
                    );
                    return Ok(());
                }
                Err(error) => {
                    println!(
                        "Failed to send to node {} (attempt {}): {}. Retrying in {:?}...",
                        rank_id, attempt, error, attempt_delay
                    );
                    tokio::time::sleep(attempt_delay).await;
                    attempt += 1;
                }
            }
        }
    }

    async fn attempt_send(
        target_addr: &str,
        message_payload: &[u8],
    ) -> Result<(), IbBTcpNetworkConfigExchangerError> {
        use tokio::io::AsyncWriteExt;

        let mut stream = TcpStream::connect(target_addr)
            .await
            .map_err(|error| ConnectionError(error))?;

        // Send 4-byte length header (little-endian)
        let len_bytes = (message_payload.len() as u32).to_le_bytes();
        stream
            .write_all(&len_bytes)
            .await
            .map_err(|error| CommunicationError(error))?;

        // Send the message payload
        stream
            .write_all(message_payload)
            .await
            .map_err(|error| CommunicationError(error))?;

        // Ensure all data is sent
        stream
            .flush()
            .await
            .map_err(|error| CommunicationError(error))?;

        Ok(())
    }

    pub async fn receive_network_config(
        socket_addr: impl ToSocketAddrs,
        network_config: &IbBCheckedStaticNetworkConfig,
        exchanger_config: &IbBTcpNetworkConfigExchangerConfig,
    ) -> Result<IbBReadyNetworkConfig, IbBTcpNetworkConfigExchangerError> {
        tokio::time::timeout(
            exchanger_config.receive_timeout,
            Self::run_network_config_server(socket_addr, network_config, exchanger_config),
        )
        .await
        .unwrap_or_else(|_| {
            Err(CommunicationError(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Operation timed out",
            )))
        })
    }

    async fn run_network_config_server(
        socket_addr: impl ToSocketAddrs,
        network_config: &IbBCheckedStaticNetworkConfig,
        exchanger_config: &IbBTcpNetworkConfigExchangerConfig,
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
            let connection_timeout = exchanger_config.receive_connection_timeout;

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
                                    connection_timeout,
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
        Ok(IbBReadyNetworkConfig::new(node_config_map, rank_ids))
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
