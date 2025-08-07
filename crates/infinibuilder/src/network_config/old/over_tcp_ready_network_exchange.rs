use crate::IbBNodeTcpQpEndpointExchangeError::InvalidAddress;
use crate::tcp_ready_network_exchange::IbBNetworkTcpQpEndpointExchangeError::{
    MaxExchangeAttemptsExceeded, MaxInvalidExchangeAttemptsExceeded, PoisonedAcceptTracker,
    UnavailableRankId,
};
use crate::{
    IbBCheckedStaticNetworkConfig, IbBNodeTcpQpEndpointExchangeError,
    IbBNodeTcpQpEndpointExchanger, IbBReadyNetworkConfig, IbBReadyNodeConfig, IbBStaticNodeConfig,
};
use ibverbs::QueuePairEndpoint;
use rayon::iter::IntoParallelIterator;
use rayon::iter::ParallelIterator;
use std::collections::{HashMap, HashSet};
use std::net::ToSocketAddrs;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IbBNetworkTcpQpEndpointExchangeError {
    #[error("Unavailable rank id {0}")]
    UnavailableRankId(u32),
    #[error(transparent)]
    NodeTcpQpEndpointExchangeError(#[from] IbBNodeTcpQpEndpointExchangeError),
    #[error("Too many exchange attempts")]
    MaxExchangeAttemptsExceeded,
    #[error("Too many invalid exchanges")]
    MaxInvalidExchangeAttemptsExceeded,
    #[error("Accept tracker is poisoned")]
    PoisonedAcceptTracker,
}

#[derive(Debug, Clone)]
pub struct IbBNetworkTcpQpEndpointExchangeConfig {
    pub socket_port: u16,
    pub max_request_retries: u32,
    pub max_valid_request_retries: u32, // A request might complete but not be valid (unsuccessful spoofing)
    pub request_attempt_interval: Duration,
    pub request_timeout: Duration,
}

impl Default for IbBNetworkTcpQpEndpointExchangeConfig {
    fn default() -> Self {
        Self {
            socket_port: 8844,
            max_request_retries: 10,
            max_valid_request_retries: 3,
            request_attempt_interval: Duration::from_millis(1000),
            request_timeout: Duration::from_millis(1000),
        }
    }
}

// Shared state to track which nodes we've successfully exchanged with
#[derive(Debug)]
struct AcceptExchangeTracker {
    missing_higher_rank_ids: HashSet<u32>,
}

impl AcceptExchangeTracker {
    fn new(higher_rank_nodes: &[IbBStaticNodeConfig]) -> Self {
        Self {
            missing_higher_rank_ids: higher_rank_nodes
                .iter()
                .map(|node| node.rank_id())
                .collect(),
        }
    }

    fn add_exchange(&mut self, remote_ready_node: &IbBReadyNodeConfig) -> bool {
        self.missing_higher_rank_ids
            .remove(&remote_ready_node.rank_id())
    }

    fn finished(&self) -> bool {
        self.missing_higher_rank_ids.is_empty()
    }
}

enum ExchangeTask {
    ConnectTo {
        target_node: IbBStaticNodeConfig,
        local_node: IbBStaticNodeConfig,
        local_qp_endpoint: QueuePairEndpoint,
        config: IbBNetworkTcpQpEndpointExchangeConfig,
    },
    AcceptAny {
        listener: Arc<IbBNodeTcpQpEndpointExchanger>,
        tracker: Arc<Mutex<AcceptExchangeTracker>>,
        local_node: IbBStaticNodeConfig,
        local_qp_endpoint: QueuePairEndpoint,
        config: IbBNetworkTcpQpEndpointExchangeConfig,
    },
}

impl ExchangeTask {
    fn execute(self) -> Result<IbBReadyNodeConfig, IbBNetworkTcpQpEndpointExchangeError> {
        match self {
            ExchangeTask::ConnectTo {
                target_node,
                local_node,
                local_qp_endpoint,
                config,
            } => Self::connect_exchange_endpoints_with_retries_and_validation(
                &target_node,
                &local_node,
                local_qp_endpoint,
                &config,
            ),
            ExchangeTask::AcceptAny {
                listener,
                tracker,
                local_node,
                local_qp_endpoint,
                config,
            } => Self::accept_exchange_endpoints_with_validation(
                &listener,
                tracker,
                local_node,
                local_qp_endpoint,
                &config,
            ),
        }
    }

    fn connect_exchange_endpoints_with_retries_and_validation(
        target_node: &IbBStaticNodeConfig,
        local_node: &IbBStaticNodeConfig,
        local_qp: QueuePairEndpoint,
        config: &IbBNetworkTcpQpEndpointExchangeConfig,
    ) -> Result<IbBReadyNodeConfig, IbBNetworkTcpQpEndpointExchangeError> {
        let mut validate_attempt = 0;
        while validate_attempt < config.max_request_retries {
            let remote_ready_node = Self::connect_exchange_endpoints_with_retries(
                target_node,
                local_node,
                local_qp,
                config,
            )?;

            if remote_ready_node.node_config == *target_node {
                return Ok(remote_ready_node);
            }

            validate_attempt += 1;
        }

        Err(MaxInvalidExchangeAttemptsExceeded)
    }

    fn connect_exchange_endpoints_with_retries(
        target_node: &IbBStaticNodeConfig,
        local_node: &IbBStaticNodeConfig,
        local_qp: QueuePairEndpoint,
        config: &IbBNetworkTcpQpEndpointExchangeConfig,
    ) -> Result<IbBReadyNodeConfig, IbBNetworkTcpQpEndpointExchangeError> {
        let remote_address = format!("{}:{}", target_node.hostname(), config.socket_port);
        let remote_address = remote_address
            .to_socket_addrs()
            .map_err(|_| InvalidAddress)?
            .next()
            .ok_or(InvalidAddress)?;

        let mut retry_attempt = 0;
        while retry_attempt < config.max_request_retries {
            match IbBNodeTcpQpEndpointExchanger::connect_and_exchange(
                remote_address,
                local_node.clone(),
                local_qp,
                config.request_timeout,
            ) {
                Ok(remote_ready_node) => return Ok(remote_ready_node),
                Err(_) => retry_attempt += 1,
            };
        }

        Err(MaxExchangeAttemptsExceeded)
    }

    fn accept_exchange_endpoints_with_validation(
        listener: &Arc<IbBNodeTcpQpEndpointExchanger>,
        tracker: Arc<Mutex<AcceptExchangeTracker>>,
        local_node: IbBStaticNodeConfig,
        local_qp_endpoint: QueuePairEndpoint,
        config: &IbBNetworkTcpQpEndpointExchangeConfig,
    ) -> Result<IbBReadyNodeConfig, IbBNetworkTcpQpEndpointExchangeError> {
        let mut validate_attempt = 0;
        while validate_attempt < config.max_request_retries {
            let remote_ready_node = Self::accept_exchange_endpoints(
                listener,
                local_node.clone(),
                local_qp_endpoint,
                config,
            )?;

            if tracker
                .lock()
                .map_err(|error| PoisonedAcceptTracker)?
                .add_exchange(&remote_ready_node)
            {
                return Ok(remote_ready_node);
            }

            validate_attempt += 1;
        }

        Err(MaxInvalidExchangeAttemptsExceeded)
    }

    fn accept_exchange_endpoints(
        listener: &Arc<IbBNodeTcpQpEndpointExchanger>,
        local_node: IbBStaticNodeConfig,
        local_qp_endpoint: QueuePairEndpoint,
        config: &IbBNetworkTcpQpEndpointExchangeConfig,
    ) -> Result<IbBReadyNodeConfig, IbBNetworkTcpQpEndpointExchangeError> {
        listener
            .accept_and_exchange(local_node, local_qp_endpoint, config.request_timeout)
            .map_err(|error| {
                IbBNetworkTcpQpEndpointExchangeError::NodeTcpQpEndpointExchangeError(error)
            })
    }
}

impl IbBCheckedStaticNetworkConfig {
    pub fn exchange_qp_endpoints(
        &self,
        local_rank_id: u32,
        local_qp_endpoint: QueuePairEndpoint,
    ) -> Result<IbBReadyNetworkConfig, IbBNetworkTcpQpEndpointExchangeError> {
        self.exchange_qp_endpoints_with_config(
            local_rank_id,
            local_qp_endpoint,
            IbBNetworkTcpQpEndpointExchangeConfig::default(),
        )
    }

    pub fn exchange_qp_endpoints_with_config(
        &self,
        local_rank_id: u32,
        local_qp_endpoint: QueuePairEndpoint,
        config: IbBNetworkTcpQpEndpointExchangeConfig,
    ) -> Result<IbBReadyNetworkConfig, IbBNetworkTcpQpEndpointExchangeError> {
        let local_node = self
            .get(&local_rank_id)
            .ok_or(UnavailableRankId(local_rank_id))?;
        let local_address = format!("{}:{}", local_node.hostname(), config.socket_port);

        // Set up TCP listener for incoming endpoint exchange requests
        let listener = Arc::new(IbBNodeTcpQpEndpointExchanger::new(local_address)?);

        // Separate nodes depending on task
        let (lower_rank_nodes, higher_rank_nodes): (Vec<_>, Vec<_>) = self
            .iter()
            .cloned()
            .filter(|node| node.rank_id() != local_rank_id)
            .partition(|node| node.rank_id() < local_rank_id);

        // Generate accepted nodes tracker
        let accept_tracker = Arc::new(Mutex::new(AcceptExchangeTracker::new(
            higher_rank_nodes.as_slice(),
        )));

        // Generate exchange task list
        let exchange_tasks = lower_rank_nodes
            .into_iter()
            .map(|node| ExchangeTask::ConnectTo {
                target_node: node,
                local_node: local_node.clone(),
                local_qp_endpoint,
                config: config.clone(),
            })
            .chain(
                higher_rank_nodes
                    .into_iter()
                    .map(|node| ExchangeTask::AcceptAny {
                        listener: listener.clone(),
                        tracker: accept_tracker.clone(),
                        local_node: local_node.clone(),
                        local_qp_endpoint,
                        config: config.clone(),
                    }),
            )
            .collect::<Vec<_>>();

        // Execute all tasks in parallel using Rayon
        let results: Vec<Result<IbBReadyNodeConfig, IbBNetworkTcpQpEndpointExchangeError>> =
            exchange_tasks
                .into_par_iter()
                .map(|task| task.execute())
                .collect();

        todo!()
    }
}
