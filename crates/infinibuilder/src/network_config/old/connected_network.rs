//! QueuePair Endpoint Exchange Module
//!
//! This module facilitates the exchange of InfiniBand QueuePair (QP) endpoint information
//! between nodes in a distributed system. The exchange happens over TCP sockets and is a
//! prerequisite for establishing actual InfiniBand RDMA connections.
//!
//! The exchange follows a rank-based client/server model:
//! - Nodes connect as TCP clients to all lower-ranked nodes
//! - Nodes accept TCP connections from all higher-ranked nodes
//!
//! Once QP endpoints are exchanged, the actual IB connections can be established elsewhere.

use crate::network::{IbBNetworkConfig, IbBNodeConfig};
use crate::{IbBEndpointExchange, IbBEndpointExchangeError};
use ibverbs::QueuePairEndpoint;
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;

// ===== Configuration Constants =====
const LISTENER_PORT: u16 = 9999;
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(100);
const MAX_RETRY_ATTEMPTS: usize = 50;
const RETRY_DELAY: Duration = Duration::from_millis(1000);

// ===== Main Network Configuration =====

/// Represents a network configuration after QueuePair endpoint exchange.
/// This contains the QP endpoint information needed to establish actual InfiniBand connections.
#[derive(Debug, Clone)]
pub struct IbBReadyNetworkConfig {
    nodes: Vec<IbBReadyNodeConfig>,
}

impl IbBReadyNetworkConfig {
    /// Returns a slice of all nodes with exchanged QP endpoints
    pub fn nodes(&self) -> &[IbBReadyNodeConfig] {
        self.nodes.as_slice()
    }
}

impl Deref for IbBReadyNetworkConfig {
    type Target = [IbBReadyNodeConfig];

    fn deref(&self) -> &Self::Target {
        self.nodes()
    }
}

impl IbBNetworkConfig {
    /// Exchanges QueuePair endpoint information with all peer nodes using TCP sockets.
    /// This exchange is a prerequisite for establishing actual InfiniBand connections.
    ///
    /// # Exchange Strategy:
    /// - Lower-rank nodes: Act as TCP clients and connect to exchange QP info
    /// - Higher-rank nodes: Act as TCP servers and accept connections to exchange QP info
    ///
    /// # Arguments
    /// * `local_rank` - The rank ID of the current node
    /// * `local_qp` - The local QueuePair endpoint to share with peers
    ///
    /// # Returns
    /// A configuration containing all peer nodes with their QP endpoint information,
    /// which can later be used to establish actual InfiniBand connections.
    pub fn exchange_qp_endpoints(
        self,
        local_rank: u32,
        local_qp: QueuePairEndpoint,
    ) -> Result<IbBReadyNetworkConfig, IbBEndpointExchangeError> {
        // Set up TCP listener for incoming endpoint exchange requests
        let listener = Self::create_exchange_listener()?;

        // Exchange QP info with lower-rank nodes (node acts as TCP client)
        let client_exchanges = self.exchange_with_lower_rank_nodes(local_rank, &local_qp)?;

        // Accept QP info from higher-rank nodes (node acts as TCP server)
        let server_exchanges = self.accept_exchanges_from_higher_rank_nodes(
            local_rank,
            &local_qp,
            Arc::clone(&listener)
        )?;

        // Combine all exchanged endpoint information
        let mut all_exchanges = client_exchanges;
        all_exchanges.extend(server_exchanges);

        Ok(IbBReadyNetworkConfig {
            nodes: all_exchanges,
        })
    }

    /// Creates a TCP listener for incoming QP endpoint exchange requests
    fn create_exchange_listener() -> Result<Arc<IbBEndpointExchange>, IbBEndpointExchangeError> {
        let listener_addr = format!("0.0.0.0:{}", LISTENER_PORT);
        let listener = IbBEndpointExchange::new(&listener_addr)?;
        Ok(Arc::new(listener))
    }

    /// Initiates QP endpoint exchange with all nodes that have lower rank
    fn exchange_with_lower_rank_nodes(
        &self,
        local_rank: u32,
        local_qp: &QueuePairEndpoint,
    ) -> Result<Vec<IbBReadyNodeConfig>, IbBEndpointExchangeError> {
        let lower_rank_nodes: Vec<_> = self
            .nodes
            .iter()
            .filter(|node| node.rank_id() < local_rank)
            .cloned()
            .collect();

        // Exchange with each lower-rank node in parallel
        lower_rank_nodes
            .par_iter()
            .map(|node| Self::exchange_with_node(node, local_qp.clone()))
            .collect::<Result<Vec<_>, IbBEndpointExchangeError>>()
    }

    /// Accepts QP endpoint exchanges from all nodes with higher rank
    fn accept_exchanges_from_higher_rank_nodes(
        &self,
        local_rank: u32,
        local_qp: &QueuePairEndpoint,
        listener: Arc<IbBEndpointExchange>,
    ) -> Result<Vec<IbBReadyNodeConfig>, IbBEndpointExchangeError> {
        let higher_rank_count = self
            .nodes
            .iter()
            .filter(|node| node.rank_id() > local_rank)
            .count();

        // Accept endpoint exchanges in parallel
        (0..higher_rank_count)
            .into_par_iter()
            .map_init(
                || Arc::clone(&listener),
                |listener, _| Self::accept_endpoint_exchange(listener, local_qp.clone()),
            )
            .collect::<Result<Vec<_>, IbBEndpointExchangeError>>()
    }

    /// Exchanges QP endpoint info with a specific node via TCP socket
    fn exchange_with_node(
        node: &IbBNodeConfig,
        local_qp: QueuePairEndpoint,
    ) -> Result<IbBReadyNodeConfig, IbBEndpointExchangeError> {
        let peer_addr = format!("{}:{}", node.hostname(), LISTENER_PORT);

        let remote_qp = Self::exchange_endpoints_with_retries(
            &peer_addr,
            local_qp,
            MAX_RETRY_ATTEMPTS,
            RETRY_DELAY,
        )?;

        Ok(IbBReadyNodeConfig {
            node_config: node.clone(),
            qp_endpoint: remote_qp,
        })
    }

    /// Accepts a single incoming QP endpoint exchange request
    fn accept_endpoint_exchange(
        listener: &Arc<IbBEndpointExchange>,
        local_qp: QueuePairEndpoint,
    ) -> Result<IbBReadyNodeConfig, IbBEndpointExchangeError> {
        let remote_qp = listener.accept_and_exchange(local_qp, CONNECTION_TIMEOUT)?;

        // Note: We don't know the remote node's details at this point
        // since we're just accepting the TCP connection for exchange
        let placeholder_config = IbBNodeConfig::new::<String>(
            "<unknown>".into(),
            "".into(),
            LISTENER_PORT as u32,
            "".into(),
        );

        Ok(IbBReadyNodeConfig {
            node_config: placeholder_config,
            qp_endpoint: remote_qp,
        })
    }

    /// Attempts to exchange QP endpoints with a peer, with retry logic
    fn exchange_endpoints_with_retries(
        addr: &str,
        local_qp: QueuePairEndpoint,
        max_attempts: usize,
        retry_delay: Duration,
    ) -> Result<QueuePairEndpoint, IbBEndpointExchangeError> {
        for attempt in 1..=max_attempts {
            match Self::try_endpoint_exchange(addr, local_qp.clone()) {
                Ok(qp) => return Ok(qp),
                Err(err) => {
                    if attempt == max_attempts {
                        eprintln!(
                            "[ERROR] Failed to exchange QP endpoints with {} after {} attempts",
                            addr, max_attempts
                        );
                        return Err(err);
                    }

                    eprintln!(
                        "[WARN] QP endpoint exchange with {} failed (attempt {}/{}): {}. \
                         Retrying in {:?}...",
                        addr, attempt, max_attempts, err, retry_delay
                    );

                    std::thread::sleep(retry_delay);
                }
            }
        }

        unreachable!("Loop should have returned by now")
    }

    /// Attempts a single QP endpoint exchange with a peer via TCP
    fn try_endpoint_exchange(
        addr: &str,
        local_qp: QueuePairEndpoint,
    ) -> Result<QueuePairEndpoint, IbBEndpointExchangeError> {
        IbBEndpointExchange::connect_and_exchange(
            addr,
            local_qp,
            CONNECTION_TIMEOUT,
        )
    }
}

// ===== Node with Exchanged QP Endpoint =====

/// Represents a single node with exchanged QueuePair endpoint information.
/// The actual InfiniBand connection can be established later using the stored QP endpoint.
#[derive(Debug, Clone)]
pub struct IbBReadyNodeConfig {
    node_config: IbBNodeConfig,
    qp_endpoint: QueuePairEndpoint,
}

impl IbBReadyNodeConfig {
    /// Returns the remote node's QueuePair endpoint that was exchanged via TCP.
    /// This endpoint can be used to establish an actual InfiniBand connection.
    pub fn qp_endpoint(&self) -> QueuePairEndpoint {
        self.qp_endpoint
    }
}

impl Deref for IbBReadyNodeConfig {
    type Target = IbBNodeConfig;

    fn deref(&self) -> &Self::Target {
        &self.node_config
    }
}