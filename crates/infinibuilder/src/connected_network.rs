use crate::network::{IbBNetworkConfig, IbBNodeConfig};
use crate::{IbBEndpointExchange, IbBEndpointExchangeError};
use ibverbs::QueuePairEndpoint;
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use std::net::TcpListener;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct IbBConnectedNetworkConfig {
    nodes: Vec<IbBConnectedNodeConfig>,
}

impl IbBNetworkConfig {
    pub fn connect_infiniband(
        self,
        local_rank: u32,
        local_qp: QueuePairEndpoint,
    ) -> Result<IbBConnectedNetworkConfig, IbBEndpointExchangeError> {
        let listener = IbBEndpointExchange::new("0.0.0.0:9999")?;
        let listener = Arc::new(listener);

        // === CLIENT SIDE (connect to lower-rank nodes) ===
        let lower_rank_peers: Vec<_> = self
            .nodes
            .iter()
            .filter(|n| n.rank_id() < local_rank)
            .cloned()
            .collect();

        let mut connections: Vec<_> = lower_rank_peers
            .par_iter()
            .map(|node| {
                let addr = format!("{}:9999", node.hostname());
                let remote_qp = Self::connect_and_exchange_with_retries(
                    &addr,
                    local_qp.clone(),
                    10,                         // max attempts
                    Duration::from_millis(500), // delay between retries
                )?;

                Ok(IbBConnectedNodeConfig {
                    node_config: node.clone(),
                    qp_endpoint: remote_qp,
                })
            })
            .collect::<Result<Vec<_>, IbBEndpointExchangeError>>()?;

        // === SERVER SIDE (accept from higher-rank nodes) ===
        let higher_rank_count = self
            .nodes
            .iter()
            .filter(|n| n.rank_id() > local_rank)
            .count();
        let listener = Arc::clone(&listener); // for Rayon

        let server_connections: Vec<_> = (0..higher_rank_count)
            .into_par_iter()
            .map_init(
                || Arc::clone(&listener), // give each thread access to listener
                |listener, _| {
                    let remote_qp =
                        listener.accept_and_exchange(local_qp.clone(), Duration::from_secs(100))?;
                    Ok(IbBConnectedNodeConfig {
                        node_config: IbBNodeConfig::new::<String>(
                            "<unknown>".into(),
                            "".into(),
                            9999,
                            "".into(),
                        ),
                        qp_endpoint: remote_qp,
                    })
                },
            )
            .collect::<Result<Vec<_>, IbBEndpointExchangeError>>()?;

        connections.extend(server_connections);

        Ok(IbBConnectedNetworkConfig { nodes: connections })
    }

    pub fn connect_and_exchange_with_retries(
        addr: &str,
        local_qp: QueuePairEndpoint,
        max_attempts: usize,
        retry_delay: Duration,
    ) -> Result<QueuePairEndpoint, IbBEndpointExchangeError> {
        for attempt in 1..=max_attempts {
            match IbBEndpointExchange::connect_and_exchange(
                addr,
                local_qp.clone(),
                Duration::from_secs(100),
            ) {
                Ok(qp) => {
                    // Optionally, validate qp here (handshake, etc.)
                    return Ok(qp);
                }
                Err(err) => {
                    if attempt == max_attempts {
                        return Err(err);
                    } else {
                        eprintln!(
                            "[WARN] Connection to {} failed on attempt {}/{}: {}. Retrying in {:?}...",
                            addr, attempt, max_attempts, err, retry_delay
                        );
                        std::thread::sleep(retry_delay);
                    }
                }
            }
        }

        unreachable!() // We return early or hit max_attempts
    }
}

impl IbBConnectedNetworkConfig {
    pub fn nodes(&self) -> &[IbBConnectedNodeConfig] {
        self.nodes.as_slice()
    }
}

impl Deref for IbBConnectedNetworkConfig {
    type Target = [IbBConnectedNodeConfig];

    fn deref(&self) -> &Self::Target {
        self.nodes()
    }
}

#[derive(Debug, Clone)]
pub struct IbBConnectedNodeConfig {
    node_config: IbBNodeConfig,
    qp_endpoint: QueuePairEndpoint,
}

impl Deref for IbBConnectedNodeConfig {
    type Target = IbBNodeConfig;

    fn deref(&self) -> &Self::Target {
        &self.node_config
    }
}

impl IbBConnectedNodeConfig {
    pub fn qp_endpoint(&self) -> QueuePairEndpoint {
        self.qp_endpoint
    }
}
