use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;

#[derive(Debug, Copy, Clone)]
pub enum IBNodeRole {
    Sender,
    Receiver,
}

pub struct IBNodeConfig {
    pub role: IBNodeRole,
    pub hostname: String,
    pub port: u16,
}

pub struct IBNetworkBuilder<T> {
    map: BTreeMap<T, IBNodeConfig>,
    ids: Vec<T>
}

impl<T: Ord> IBNetworkBuilder<T> {
    pub fn new() -> Self {
        Self {
            map: BTreeMap::new(),
            ids: Vec::new(),
        }
    }

    pub fn insert_node(&mut self, id: T, config: IBNodeConfig) -> Option<IBNodeConfig> {
        self.map.insert(id, config)
    }

    pub fn build(self) -> IBNetwork<T> {
        let mut node_configs = Vec::new();
        let mut indexed_nodes = BTreeMap::new();
        let mut sender_nodes = Vec::new();
        let mut receiver_nodes = Vec::new();

        for (idx, (node_id, config)) in self.map.into_iter().enumerate() {
            indexed_nodes.insert(node_id, idx);

            match config.role {
                IBNodeRole::Sender => sender_nodes.push(idx),
                IBNodeRole::Receiver => receiver_nodes.push(idx),
            }

            node_configs.push(config);
        }

        IBNetwork {
            node_configs,
            indexed_nodes,
            sender_nodes,
            receiver_nodes,
        }
    }
}

pub struct IBNetwork<T: Ord> {
    node_configs: Vec<IBNodeConfig>,
    indexed_nodes: BTreeMap<T, usize>,
    sender_nodes: Vec<usize>,
    receiver_nodes: Vec<usize>,
}

impl<T: Ord> IBNetwork<T> {
    pub fn node(&self, node_id: impl AsRef<T>) -> Option<IBNodeRole> {
        self.indexed_nodes
            .get(node_id.as_ref())
            .map(|&index| self.node_configs[index].role)
    }

    pub fn nodes(&self) -> &[IBNodeConfig] {
        self.node_configs.as_slice()
    }

    pub fn senders(&self) -> Vec<&IBNodeConfig> {
        self.sender_nodes
            .iter()
            .map(|&index| &self.node_configs[index])
            .collect()
    }

    pub fn receivers(&self) -> Vec<&IBNodeConfig> {
        self.receiver_nodes
            .iter()
            .map(|&index| &self.node_configs[index])
            .collect()
    }
}
