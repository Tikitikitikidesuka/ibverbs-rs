use crate::tcp_exchanger::TcpExchangerError::DuplicatedNodeId;
use crate::network::IBNetwork;
use futures::future::join_all;
use futures::join;
use serde::de::{DeserializeOwned, Error};
use serde::{Deserialize, Serialize};
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
pub enum TcpExchangerError {
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
    NonExistentRankId(usize),
    #[error("Duplicated node id {0} on network config")]
    DuplicatedNodeId(usize),
}

pub struct TcpExchanger<T: Serialize + DeserializeOwned> {
    _marker: std::marker::PhantomData<T>,
}

#[derive(Debug, Clone)]
pub struct TcpExchangerNetworkConfig {
    nodes: HashMap<usize, TcpExchangerNodeConfig>,
    node_ids: Vec<usize>, // To iterate efficiently
}

impl TcpExchangerNetworkConfig {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            node_ids: Vec::new(),
        }
    }

    pub fn from_network<T: Ord>(network: IBNetwork<T>) -> Result<Self, TcpExchangerError> {
        network
            .nodes()
            .iter()
            .try_fold(Self::new(), |exchanger_network, node_config| {
                exchanger_network.add_node(TcpExchangerNodeConfig::new(
                    node_config.idx,
                    node_config.address.clone(),
                    node_config.port,
                ))
            })
    }

    pub fn add_node(mut self, node: TcpExchangerNodeConfig) -> Result<Self, TcpExchangerError> {
        if !self.nodes.contains_key(&node.node_id) {
            self.node_ids.push(node.node_id);
            self.nodes.insert(node.node_id, node);
            Ok(self)
        } else {
            Err(DuplicatedNodeId(node.node_id))
        }
    }

    pub fn get(&self, node_id: &usize) -> Option<&TcpExchangerNodeConfig> {
        self.nodes.get(node_id)
    }

    pub fn len(&self) -> usize {
        self.node_ids.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &TcpExchangerNodeConfig> + '_ {
        self.node_ids
            .iter()
            .filter_map(move |id| self.nodes.get(id))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TcpExchangerNodeConfig {
    node_id: usize,
    address: String,
    port: u16,
}

impl TcpExchangerNodeConfig {
    pub fn new(node_id: usize, address: String, port: u16) -> Self {
        Self {
            node_id,
            address,
            port,
        }
    }
}

pub struct TcpExchangedData<T> {
    nodes: Vec<TcpExchangedNodeData<T>>,
}

impl<T> TcpExchangedData<T> {
    pub fn as_slice(&self) -> &[TcpExchangedNodeData<T>] {
        &self.nodes
    }

    pub fn iter(&self) -> std::slice::Iter<'_, TcpExchangedNodeData<T>> {
        self.nodes.iter()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TcpExchangedNodeData<T> {
    pub node_id: usize,
    pub data: T,
}

impl<T> TcpExchangedNodeData<T> {
    pub fn node_id(&self) -> usize {
        self.node_id
    }

    pub fn data(&self) -> &T {
        &self.data
    }
}

pub struct TcpExchangerConfig {
    pub send_timeout: Duration,               // Timeout for whole network send
    pub send_attempt_delay: Duration,         // Delay between send attempts
    pub receive_timeout: Duration,            // Timeout for whole network receive
    pub receive_connection_timeout: Duration, // Timeout per connection in receive
}

impl Default for TcpExchangerConfig {
    fn default() -> Self {
        Self {
            send_timeout: Duration::from_secs(30),
            send_attempt_delay: Duration::from_secs(1),
            receive_timeout: Duration::from_secs(60),
            receive_connection_timeout: Duration::from_secs(10),
        }
    }
}

impl<T> TcpExchanger<T>
where
    T: Serialize + DeserializeOwned + Send + Sync + 'static,
{
    pub fn await_exchange_network_config(
        node_id: usize,
        out_data: &T,
        network_config: &TcpExchangerNetworkConfig,
        exchanger_config: &TcpExchangerConfig,
    ) -> Result<TcpExchangedData<T>, TcpExchangerError> {
        Runtime::new()
            .map_err(|error| TcpExchangerError::ConnectionError(error))?
            .block_on(Self::exchange_network_config(
                node_id,
                out_data,
                network_config,
                exchanger_config,
            ))
    }

    pub async fn exchange_network_config(
        node_id: usize,
        out_data: &T,
        network_config: &TcpExchangerNetworkConfig,
        exchanger_config: &TcpExchangerConfig,
    ) -> Result<TcpExchangedData<T>, TcpExchangerError> {
        let send_fut =
            Self::send_network_config(node_id, out_data, network_config, exchanger_config);

        let tcp_node_config = network_config
            .get(&node_id)
            .ok_or(TcpExchangerError::NonExistentRankId(node_id))?;
        let socket_addr = format!("{}:{}", tcp_node_config.address, tcp_node_config.port);
        println!("Receiving at {socket_addr}");

        let recv_fut = Self::receive_network_config(socket_addr, network_config, exchanger_config);

        let (send_result, recv_result) = join!(send_fut, recv_fut);

        // Prioritize returning receive errors if they occur
        match (send_result, recv_result) {
            (Ok(_), Ok(ready_config)) => Ok(ready_config),
            (_, Err(e)) => Err(e),
            (Err(e), _) => Err(e),
        }
    }

    pub async fn send_network_config(
        node_id: usize,
        out_data: &T,
        network_config: &TcpExchangerNetworkConfig,
        exchanger_config: &TcpExchangerConfig,
    ) -> Result<(), TcpExchangerError> {
        tokio::time::timeout(
            exchanger_config.send_timeout,
            Self::send_config_to_nodes_async(node_id, out_data, network_config, exchanger_config),
        )
        .await
        .unwrap_or_else(|_| {
            Err(TcpExchangerError::CommunicationError(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Operation timed out",
            )))
        })
    }

    // Will loop retrying failed sends until all are finished successfully
    // Must be run with a timeout block to prevent infinite loop
    async fn send_config_to_nodes_async(
        node_id: usize,
        out_data: &T,
        network_config: &TcpExchangerNetworkConfig,
        exchanger_config: &TcpExchangerConfig,
    ) -> Result<(), TcpExchangerError> {
        let sent_data = TcpExchangedNodeData {
            node_id,
            data: out_data.clone(),
        };

        let message_payload =
            serde_json::to_vec(&sent_data).map_err(TcpExchangerError::InvalidMessage)?;

        if message_payload.len() > MAX_MESSAGE_LENGTH {
            return Err(TcpExchangerError::MessageTooLarge(message_payload.len()));
        }

        // Create tasks for sending to all nodes concurrently
        let mut send_tasks = Vec::new();

        for node_config in network_config.iter() {
            let target_node_id = node_config.node_id;
            let address = node_config.address.clone();
            let tcp_port = node_config.port;
            let attempt_delay = exchanger_config.send_attempt_delay;
            let message_payload = message_payload.clone();

            let task = tokio::spawn(async move {
                Self::loop_attempt_send_to_single_node(
                    &address,
                    tcp_port,
                    &message_payload,
                    attempt_delay,
                    target_node_id,
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
                return Err(TcpExchangerError::RuntimeServerError(format!(
                    "Task join error: {}",
                    join_error
                )));
            }
        }

        Ok(())
    }

    async fn loop_attempt_send_to_single_node(
        address: &str,
        tcp_port: u16,
        message_payload: &[u8],
        attempt_delay: Duration,
        node_id: usize,
    ) -> Result<(), TcpExchangerError> {
        let target_addr = format!("{}:{}", address, tcp_port);
        let mut attempt = 1;

        loop {
            match Self::attempt_send(&target_addr, message_payload).await {
                Ok(()) => {
                    println!(
                        "Successfully sent config to node {} ({})",
                        node_id, target_addr
                    );
                    return Ok(());
                }
                Err(error) => {
                    println!(
                        "Failed to send to node {} (attempt {}): {}. Retrying in {:?}...",
                        node_id, attempt, error, attempt_delay
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
    ) -> Result<(), TcpExchangerError> {
        use tokio::io::AsyncWriteExt;

        let mut stream = TcpStream::connect(target_addr)
            .await
            .map_err(TcpExchangerError::ConnectionError)?;

        // Send 4-byte length header (little-endian)
        let len_bytes = (message_payload.len() as usize).to_le_bytes();
        stream
            .write_all(&len_bytes)
            .await
            .map_err(TcpExchangerError::CommunicationError)?;

        // Send the message payload
        stream
            .write_all(message_payload)
            .await
            .map_err(TcpExchangerError::CommunicationError)?;

        // Ensure all data is sent
        stream
            .flush()
            .await
            .map_err(TcpExchangerError::CommunicationError)?;

        Ok(())
    }

    pub async fn receive_network_config(
        socket_addr: impl ToSocketAddrs,
        network_config: &TcpExchangerNetworkConfig,
        exchanger_config: &TcpExchangerConfig,
    ) -> Result<TcpExchangedData<T>, TcpExchangerError> {
        tokio::time::timeout(
            exchanger_config.receive_timeout,
            Self::run_network_config_server(socket_addr, network_config, exchanger_config),
        )
        .await
        .unwrap_or_else(|_| {
            Err(TcpExchangerError::CommunicationError(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Operation timed out",
            )))
        })
    }

    async fn run_network_config_server(
        socket_addr: impl ToSocketAddrs,
        network_config: &TcpExchangerNetworkConfig,
        exchanger_config: &TcpExchangerConfig,
    ) -> Result<TcpExchangedData<T>, TcpExchangerError> {
        let listener = TcpListener::bind(socket_addr)
            .await
            .map_err(TcpExchangerError::ConnectionError)?;

        let total_nodes = network_config.nodes.len();

        // Shared state for tracking received nodes by node id
        let received_node_ids = Arc::new(Mutex::new(HashSet::new()));
        let network_config = Arc::new(network_config.clone());

        // Channel for sending received ready nodes to main task
        let (tx, mut rx) =
            mpsc::channel::<Result<TcpExchangedNodeData<T>, TcpExchangerError>>(total_nodes);

        // Spawn the connection acceptor task
        let listener_task = {
            let tx = tx.clone();
            let network_config = Arc::clone(&network_config);
            let received_node_ids = Arc::clone(&received_node_ids);
            let connection_timeout = exchanger_config.receive_connection_timeout;

            tokio::spawn(async move {
                loop {
                    match listener.accept().await {
                        Ok((stream, peer_addr)) => {
                            println!("Received connection from: {}", peer_addr);

                            // Spawn a task to handle each connection in parallel
                            let tx_clone = tx.clone();
                            let network_config_clone = Arc::clone(&network_config);
                            let received_node_ids_clone = Arc::clone(&received_node_ids);

                            tokio::spawn(async move {
                                let result = Self::handle_connection(
                                    stream,
                                    &network_config_clone,
                                    &received_node_ids_clone,
                                    connection_timeout,
                                )
                                .await;

                                // Send result back to main task
                                if tx_clone.send(result).await.is_err() {
                                    println!(
                                        "Failed to send result for connection from {}",
                                        peer_addr
                                    );
                                }
                            });
                        }
                        Err(accept_error) => {
                            println!("Failed to accept connection: {}", accept_error);
                            let _ = tx
                                .send(Err(TcpExchangerError::ConnectionError(accept_error)))
                                .await;
                            break;
                        }
                    }
                }
            })
        };

        // Collect results from parallel connection handlers
        let mut successful_nodes = 0;
        let mut in_data_vec = Vec::new();

        while successful_nodes < total_nodes {
            match rx.recv().await {
                Some(Ok(in_data)) => {
                    successful_nodes += 1;
                    in_data_vec.push(in_data);
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
                    return Err(TcpExchangerError::RuntimeServerError(
                        "Channel closed unexpectedly".to_string(),
                    ));
                }
            }
        }

        // Cancel the listener task since we have all nodes
        listener_task.abort();

        println!("All {} nodes connected successfully!", total_nodes);

        // Sort input data by node id
        in_data_vec.sort_by(|a, b| a.node_id.cmp(&b.node_id));

        // Create and return the ready network config
        Ok(TcpExchangedData { nodes: in_data_vec })
    }

    async fn handle_connection(
        mut stream: TcpStream,
        network_config: &TcpExchangerNetworkConfig,
        received_nodes: &Arc<Mutex<HashSet<usize>>>,
        timeout: Duration,
    ) -> Result<TcpExchangedNodeData<T>, TcpExchangerError> {
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
                .insert(validated_ready_node.node_id());

            // Return the ready node config
            Ok(validated_ready_node)
        })
        .await;

        result.unwrap_or_else(|_| {
            Err(TcpExchangerError::CommunicationError(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Operation timed out",
            )))
        })
    }

    async fn validate_ready_node_from_config(
        ready_node: TcpExchangedNodeData<T>,
        network_config: &TcpExchangerNetworkConfig,
        received_nodes: &Arc<Mutex<HashSet<usize>>>,
    ) -> Result<TcpExchangedNodeData<T>, TcpExchangerError> {
        let node_id = ready_node.node_id();

        if received_nodes.lock().await.contains(&node_id) {
            return Err(TcpExchangerError::InvalidMessage(
                serde_json::Error::custom("Node already received"),
            ));
        }

        // Check if node_id exists in network config
        match network_config.get(&node_id) {
            Some(_) => Ok(ready_node),
            None => Err(TcpExchangerError::NonExistentRankId(node_id)),
        }
    }

    async fn read_ready_node_from_stream(
        stream: &mut TcpStream,
        buffer: &mut [u8],
    ) -> Result<TcpExchangedNodeData<T>, TcpExchangerError> {
        // Read message size from 4 byte header
        let mut len_bytes = [0u8; size_of::<usize>()];
        stream
            .read_exact(&mut len_bytes)
            .await
            .map_err(TcpExchangerError::CommunicationError)?;

        let len = usize::from_le_bytes(len_bytes);

        // Check size is within buffer boundaries
        if len > MAX_MESSAGE_LENGTH {
            return Err(TcpExchangerError::MessageTooLarge(len));
        }

        // Read only the required bytes from the stream
        let msg_buffer = &mut buffer[..len];
        stream
            .read_exact(msg_buffer)
            .await
            .map_err(TcpExchangerError::CommunicationError)?;

        // Deserialize message payload
        let config =
            serde_json::from_slice(msg_buffer).map_err(TcpExchangerError::InvalidMessage)?;

        Ok(config)
    }
}
