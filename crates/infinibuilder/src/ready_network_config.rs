use crate::IbBEndpointExchangeError::ConnectionError;
use crate::network_config::IbBStaticNodeConfig;
use crate::{IbBCheckedStaticNetworkConfig, IbBEndpointExchange, IbBEndpointExchangeError};
use ibverbs::QueuePairEndpoint;
use rayon::iter::IntoParallelIterator;
use rayon::iter::ParallelIterator;
use std::net::ToSocketAddrs;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct IbBReadyNetworkConfig {
    node_config_vec: Vec<IbBReadyNodeConfig>,
}

#[derive(Debug, Clone)]
pub struct IbBReadyNodeConfig {
    node_config: IbBStaticNodeConfig,
    qp_endpoint: QueuePairEndpoint,
}

impl Deref for IbBReadyNetworkConfig {
    type Target = [IbBReadyNodeConfig];

    fn deref(&self) -> &Self::Target {
        self.node_config_vec.as_slice()
    }
}

impl Deref for IbBReadyNodeConfig {
    type Target = IbBStaticNodeConfig;

    fn deref(&self) -> &Self::Target {
        &self.node_config
    }
}

impl IbBReadyNodeConfig {
    pub fn qp_endpoint(&self) -> QueuePairEndpoint {
        self.qp_endpoint
    }
}

#[derive(Debug, Clone)]
pub struct IbBNetworkQpEndpointExchangeConfig {
    pub socket_port: u16,
    pub max_request_retries: u32,
    pub request_attempt_interval: Duration,
    pub request_timeout: Duration,
}

impl Default for IbBNetworkQpEndpointExchangeConfig {
    fn default() -> Self {
        Self {
            socket_port: 8844,
            max_request_retries: 10,
            request_attempt_interval: Duration::from_millis(1000),
            request_timeout: Duration::from_millis(1000),
        }
    }
}

impl IbBCheckedStaticNetworkConfig {
    pub fn exchange_qp_endpoints(
        self,
        local_rank: u32,
        local_qp: QueuePairEndpoint,
    ) -> Result<IbBReadyNetworkConfig, (Self, String)> {
        self.exchange_qp_endpoints_with_config(
            local_rank,
            local_qp,
            IbBNetworkQpEndpointExchangeConfig::default(),
        )
    }

    pub fn exchange_qp_endpoints_with_config(
        self,
        local_rank: u32,
        local_qp: QueuePairEndpoint,
        config: IbBNetworkQpEndpointExchangeConfig,
    ) -> Result<IbBReadyNetworkConfig, (Self, String)> {
        let local_node = match self.get(&local_rank) {
            None => {
                return Err((
                    self,
                    "Node not found TODO: BETTER ERROR HANDLING".to_string(),
                ));
            }
            Some(local_node) => local_node,
        };

        let local_address = format!("{}:{}", local_node.hostname(), config.socket_port);
        println!("Address: {}", local_address);

        // Set up TCP listener for incoming endpoint exchange requests
        let listener = match IbBEndpointExchange::new(local_address) {
            Ok(l) => Arc::new(l),
            Err(e) => return Err((self, format!("Failed to create listener: {}", e))),
        };

        // Partition nodes into those with lower and higher ranks
        let (lower_rank_nodes, higher_rank_nodes): (Vec<_>, Vec<_>) = self
            .iter()
            .map(|(_rank_id, node)| node)
            .cloned()
            .filter(|node| node.rank_id() != local_rank)
            .partition(|node| node.rank_id() < local_rank);

        enum ExchangeOp {
            ConnectTo(IbBStaticNodeConfig),
            AcceptFrom(IbBStaticNodeConfig),
        }

        let all_operations = lower_rank_nodes
            .into_iter()
            .map(ExchangeOp::ConnectTo)
            .chain(higher_rank_nodes.into_iter().map(ExchangeOp::AcceptFrom))
            .collect::<Vec<_>>();

        // Perform all exchanges in parallel
        let results: Result<Vec<IbBReadyNodeConfig>, IbBEndpointExchangeError> = all_operations
            .into_par_iter()
            .map(|op| match op {
                ExchangeOp::ConnectTo(node) => {
                    println!("Connect to {:?}", node.rank_id());
                    let addr = format!("{}:{}", node.hostname(), config.socket_port);
                    let qp_endpoint = Self::exchange_endpoints_with_retries(
                        &addr,
                        local_qp.clone(),
                        config.max_request_retries,
                        config.request_attempt_interval,
                        config.request_timeout,
                    )
                    .map_err(|error| {
                        println!("Connect to {:?} failed: {:?}", node.rank_id(), error);
                        error
                    })?;
                    Ok(IbBReadyNodeConfig {
                        node_config: node,
                        qp_endpoint,
                    })
                }
                ExchangeOp::AcceptFrom(node) => {
                    println!("Accept from {:?}", node.rank_id());
                    let qp_endpoint = Self::accept_endpoint_exchange(
                        &listener,
                        local_qp.clone(),
                        config.request_timeout,
                    )
                    .map_err(|error| {
                        println!("Accept from {:?} failed: {:?}", node.rank_id(), error);
                        error
                    })?;
                    Ok(IbBReadyNodeConfig {
                        node_config: node,
                        qp_endpoint,
                    })
                }
            })
            .collect();

        let node_config_vec = match results {
            Ok(vec) => vec,
            Err(e) => {
                eprintln!("[ERROR] QP exchange failed: {}", e);
                return Err((self, format!("QP exchange failed: {}", e)));
            }
        };

        Ok(IbBReadyNetworkConfig { node_config_vec })
    }

    /// Accepts a single incoming QP endpoint exchange request
    fn accept_endpoint_exchange(
        listener: &Arc<IbBEndpointExchange>,
        local_qp: QueuePairEndpoint,
        attempt_timeout: Duration,
    ) -> Result<QueuePairEndpoint, IbBEndpointExchangeError> {
        let remote_qp = listener.accept_and_exchange(local_qp, attempt_timeout)?;

        // TODO: Change this to also pass config and verify

        Ok(remote_qp)
    }

    /// Attempts to exchange QP endpoints with a peer, with retry logic
    fn exchange_endpoints_with_retries(
        addr: impl ToSocketAddrs,
        local_qp: QueuePairEndpoint,
        max_attempts: u32,
        retry_delay: Duration,
        attempt_timeout: Duration,
    ) -> Result<QueuePairEndpoint, IbBEndpointExchangeError> {
        let addr = addr
            .to_socket_addrs()?
            .next()
            .ok_or(ConnectionError(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid address",
            )))?;

        for attempt in 1..max_attempts {
            match IbBEndpointExchange::connect_and_exchange(addr, local_qp.clone(), attempt_timeout)
            {
                Ok(qp) => return Ok(qp),
                Err(err) => {
                    eprintln!(
                        "[WARN] QP endpoint exchange with {} failed (attempt {}/{}): {}. \
                     Retrying in {:?}...",
                        addr, attempt, max_attempts, err, retry_delay
                    );
                    std::thread::sleep(retry_delay);
                }
            }
        }

        // Final attempt (no sleep or retry afterward)
        match IbBEndpointExchange::connect_and_exchange(addr, local_qp, attempt_timeout) {
            Ok(qp) => Ok(qp),
            Err(err) => {
                eprintln!(
                    "[ERROR] Failed to exchange QP endpoints with {} after {} attempts",
                    addr, max_attempts
                );
                Err(err)
            }
        }
    }
}
