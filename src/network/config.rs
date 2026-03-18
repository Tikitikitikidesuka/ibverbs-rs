use bon::Builder;
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct NetworkConfig {
    hosts: Vec<NodeConfig>,
}

#[derive(Debug, Clone, Builder, Serialize, Deserialize)]
#[builder(on(String, into))]
pub struct NodeConfig {
    pub hostname: String,
    pub port: u16,
    pub ibdev: String,
    pub rankid: usize,
    #[builder(default)]
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub comment: String,
}

impl NetworkConfig {
    pub fn builder() -> RawNetworkConfig {
        RawNetworkConfig { hosts: vec![] }
    }
}

#[derive(Debug, Copy, Clone, Error)]
pub enum NetworkConfigError {
    #[error("Empty network")]
    EmptyNetwork,
    #[error("First rank id is not zero")]
    FirstRankNotZero,
    #[error("Ranks are non sequential, {gap_rank} is missing")]
    NonSequentialRanks { gap_rank: usize },
    #[error("Rank {dup_rank} appears multiple times")]
    DuplicatedRank { dup_rank: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RawNetworkConfig {
    hosts: Vec<NodeConfig>,
}

impl RawNetworkConfig {
    pub fn add_node(mut self, node: NodeConfig) -> Self {
        self.hosts.push(node);
        self
    }

    pub fn truncate(mut self, num_nodes: usize) -> Self {
        self.hosts.truncate(num_nodes);
        self
    }

    pub fn build(mut self) -> Result<NetworkConfig, NetworkConfigError> {
        self.hosts.sort_by_key(|n| n.rankid);

        // Network cannot be empty
        if self.hosts.is_empty() {
            return Err(NetworkConfigError::EmptyNetwork);
        }

        // Rank ids must start at 0
        if self.hosts.first().map(|h| h.rankid) != Some(0) {
            return Err(NetworkConfigError::FirstRankNotZero);
        }

        for i in 1..self.hosts.len() {
            let node_config = &self.hosts[i];

            // Rank ids must be unique
            if node_config.rankid == self.hosts[i - 1].rankid {
                return Err(NetworkConfigError::DuplicatedRank {
                    dup_rank: node_config.rankid,
                });
            }

            // Rank ids must be sequential
            if node_config.rankid != i {
                return Err(NetworkConfigError::NonSequentialRanks { gap_rank: i });
            }
        }

        Ok(NetworkConfig { hosts: self.hosts })
    }
}

impl Deref for NetworkConfig {
    type Target = [NodeConfig];

    fn deref(&self) -> &Self::Target {
        self.hosts.as_slice()
    }
}

impl<'a> IntoIterator for &'a NetworkConfig {
    type Item = &'a NodeConfig;
    type IntoIter = std::slice::Iter<'a, NodeConfig>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl NetworkConfig {
    pub fn world_size(&self) -> usize {
        self.hosts.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_network_config() {
        let config_builder = RawNetworkConfig {
            hosts: vec![
                NodeConfig {
                    hostname: "tdeb02".to_string(),
                    port: 10000,
                    ibdev: "mlx5_0".to_string(),
                    rankid: 0,
                    comment: String::new(),
                },
                NodeConfig {
                    hostname: "tdeb02".to_string(),
                    port: 10001,
                    ibdev: "mlx5_0".to_string(),
                    rankid: 1,
                    comment: String::new(),
                },
            ],
        };

        let config = config_builder.build().unwrap();
        assert_eq!(config.len(), 2);
        assert_eq!(config[0].rankid, 0);
        assert_eq!(config[1].rankid, 1);
    }

    #[test]
    fn valid_network_config_out_of_order() {
        let config_builder = RawNetworkConfig {
            hosts: vec![
                NodeConfig {
                    hostname: "node2".to_string(),
                    port: 10001,
                    ibdev: "mlx5_0".to_string(),
                    rankid: 1,
                    comment: String::new(),
                },
                NodeConfig {
                    hostname: "node1".to_string(),
                    port: 10000,
                    ibdev: "mlx5_0".to_string(),
                    rankid: 0,
                    comment: String::new(),
                },
                NodeConfig {
                    hostname: "node3".to_string(),
                    port: 10002,
                    ibdev: "mlx5_0".to_string(),
                    rankid: 2,
                    comment: String::new(),
                },
            ],
        };

        let config = config_builder.build().unwrap();
        // Should be sorted by rank ID
        assert_eq!(config[0].rankid, 0);
        assert_eq!(config[0].hostname, "node1");
        assert_eq!(config[1].rankid, 1);
        assert_eq!(config[1].hostname, "node2");
        assert_eq!(config[2].rankid, 2);
        assert_eq!(config[2].hostname, "node3");
    }

    #[test]
    fn empty_node_config() {
        let config_builder = RawNetworkConfig { hosts: vec![] };
        assert!(matches!(
            config_builder.build(),
            Err(NetworkConfigError::EmptyNetwork)
        ));
    }

    #[test]
    fn single_node_config() {
        let config_builder = RawNetworkConfig {
            hosts: vec![NodeConfig {
                hostname: "single".to_string(),
                port: 8080,
                ibdev: "mlx5_1".to_string(),
                rankid: 0,
                comment: String::new(),
            }],
        };

        let config = config_builder.build().unwrap();
        assert_eq!(config.len(), 1);
        assert_eq!(config[0].rankid, 0);
    }

    #[test]
    fn missing_rank_zero() {
        let config_builder = RawNetworkConfig {
            hosts: vec![
                NodeConfig {
                    hostname: "node1".to_string(),
                    port: 10000,
                    ibdev: "mlx5_0".to_string(),
                    rankid: 1,
                    comment: String::new(),
                },
                NodeConfig {
                    hostname: "node2".to_string(),
                    port: 10001,
                    ibdev: "mlx5_0".to_string(),
                    rankid: 2,
                    comment: String::new(),
                },
            ],
        };

        assert!(matches!(
            config_builder.build(),
            Err(NetworkConfigError::FirstRankNotZero)
        ));
    }

    #[test]
    fn non_sequential_ranks() {
        let config_builder = RawNetworkConfig {
            hosts: vec![
                NodeConfig {
                    hostname: "node1".to_string(),
                    port: 10000,
                    ibdev: "mlx5_0".to_string(),
                    rankid: 0,
                    comment: String::new(),
                },
                NodeConfig {
                    hostname: "node2".to_string(),
                    port: 10001,
                    ibdev: "mlx5_0".to_string(),
                    rankid: 2, // Missing rankid 1
                    comment: String::new(),
                },
            ],
        };

        assert!(matches!(
            config_builder.build(),
            Err(NetworkConfigError::NonSequentialRanks { gap_rank: 1 })
        ));
    }

    #[test]
    fn non_sequential_ranks_before_duplicate() {
        // Gap at rankid 1, duplicate at rankid 3
        // Gap should be detected first since 1 < 3
        let config_builder = RawNetworkConfig {
            hosts: vec![
                NodeConfig {
                    hostname: "node1".to_string(),
                    port: 10000,
                    ibdev: "mlx5_0".to_string(),
                    rankid: 0,
                    comment: String::new(),
                },
                NodeConfig {
                    hostname: "node2".to_string(),
                    port: 10001,
                    ibdev: "mlx5_0".to_string(),
                    rankid: 3, // Gap: missing rankid 1 and 2
                    comment: String::new(),
                },
                NodeConfig {
                    hostname: "node3".to_string(),
                    port: 10002,
                    ibdev: "mlx5_0".to_string(),
                    rankid: 3, // Duplicate rankid 3
                    comment: String::new(),
                },
            ],
        };

        assert!(matches!(
            config_builder.build(),
            Err(NetworkConfigError::NonSequentialRanks { gap_rank: 1 })
        ));
    }

    #[test]
    fn duplicate_ranks_before_non_sequential() {
        // Duplicate at rankid 1, gap at rankid 3 (missing 2)
        // Duplicate should be detected first since 1 < 3
        let config_builder = RawNetworkConfig {
            hosts: vec![
                NodeConfig {
                    hostname: "node1".to_string(),
                    port: 10000,
                    ibdev: "mlx5_0".to_string(),
                    rankid: 0,
                    comment: String::new(),
                },
                NodeConfig {
                    hostname: "node2".to_string(),
                    port: 10001,
                    ibdev: "mlx5_0".to_string(),
                    rankid: 1,
                    comment: String::new(),
                },
                NodeConfig {
                    hostname: "node3".to_string(),
                    port: 10002,
                    ibdev: "mlx5_0".to_string(),
                    rankid: 1, // Duplicate rankid 1
                    comment: String::new(),
                },
                NodeConfig {
                    hostname: "node4".to_string(),
                    port: 10003,
                    ibdev: "mlx5_0".to_string(),
                    rankid: 3, // Gap: missing rankid 2
                    comment: String::new(),
                },
            ],
        };

        assert!(matches!(
            config_builder.build(),
            Err(NetworkConfigError::DuplicatedRank { dup_rank: 1 })
        ));
    }

    #[test]
    fn deref_access() {
        let config_builder = RawNetworkConfig {
            hosts: vec![
                NodeConfig {
                    hostname: "test1".to_string(),
                    port: 9000,
                    ibdev: "mlx5_0".to_string(),
                    rankid: 0,
                    comment: String::new(),
                },
                NodeConfig {
                    hostname: "test2".to_string(),
                    port: 9001,
                    ibdev: "mlx5_0".to_string(),
                    rankid: 1,
                    comment: String::new(),
                },
            ],
        };

        let config = config_builder.build().unwrap();

        // Test Deref implementation - should work like a slice
        assert_eq!(config.len(), 2);
        assert_eq!(config[0].hostname, "test1");
        assert_eq!(config[1].hostname, "test2");
        assert_eq!(config.first().unwrap().port, 9000);
        assert_eq!(config.last().unwrap().port, 9001);

        // Test iteration
        let hostnames: Vec<&String> = config.iter().map(|node| &node.hostname).collect();
        assert_eq!(hostnames, vec!["test1", "test2"]);
    }
}
