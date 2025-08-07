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
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
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

        // Separate nodes by task depending on rank id
        let (connect_nodes, accept_nodes): (Vec<_>, Vec<_>) = self
            .iter()
            .cloned()
            .filter(|node| node.rank_id() != local_rank_id)
            .partition(|node| node.rank_id() < local_rank_id);

        // Channel to collect all results
        let (tx, rx) = mpsc::channel();
        let expected_total = connect_nodes.len() + accept_nodes.len();

        // Thread 1: Handle all connects
        let connect_tx = tx.clone();
        let connect_handle = if !connect_nodes.is_empty() {
            let local_node = local_node.clone();
            let config = config.clone();
            Some(thread::spawn(move || {
                // Use Rayon for parallel connects
                let results: Vec<Result<IbBReadyNodeConfig, IbBNetworkTcpQpEndpointExchangeError>> =
                    connect_nodes
                        .into_par_iter()
                        .map(|(_, target_node)| {
                            Self::connect_to_node(target_node, &local_node, local_qp_endpoint, &config)
                        })
                        .collect();

                // Send each result
                for result in results {
                    if connect_tx.send(result).is_err() {
                        eprintln!("Failed to send connect result");
                        break;
                    }
                }
                println!("Connect thread finished");
            }))
        } else {
            None
        };

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
