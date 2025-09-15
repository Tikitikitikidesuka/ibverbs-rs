use std::borrow::Borrow;
use std::collections::BTreeMap;

#[derive(Debug, Copy, Clone)]
pub enum IBNodeRole {
    Sender,
    Receiver,
}

pub struct IBNodeBuilderConfig {
    pub role: IBNodeRole,
    pub address: String,
    pub port: u16,
}

pub struct IBNetworkBuilder<T> {
    map: BTreeMap<T, IBNodeBuilderConfig>,
    ids: Vec<T>,
}

impl<T: Ord + Clone> IBNetworkBuilder<T> {
    pub fn new() -> Self {
        Self {
            map: BTreeMap::new(),
            ids: Vec::new(),
        }
    }

    pub fn insert_node(
        &mut self,
        id: T,
        config: IBNodeBuilderConfig,
    ) -> Option<IBNodeBuilderConfig> {
        self.map.insert(id, config)
    }

    pub fn build(self) -> IBNetwork<T> {
        let mut node_configs = Vec::new();
        let mut indexed_nodes = BTreeMap::new();
        let mut sender_nodes = Vec::new();
        let mut receiver_nodes = Vec::new();

        for (idx, (node_id, config)) in self.map.into_iter().enumerate() {
            indexed_nodes.insert(node_id.clone(), idx);

            match config.role {
                IBNodeRole::Sender => sender_nodes.push(idx),
                IBNodeRole::Receiver => receiver_nodes.push(idx),
            }

            node_configs.push(IBNodeConfig {
                id: node_id,
                idx,
                role: config.role,
                address: config.address,
                port: config.port,
            });
        }

        IBNetwork {
            node_configs,
            indexed_nodes,
            sender_nodes,
            receiver_nodes,
        }
    }
}

pub struct IBNodeConfig<T> {
    pub id: T,
    pub idx: usize,
    pub role: IBNodeRole,
    pub address: String,
    pub port: u16,
}

/// Every node is identified by a numerical index assigned in an ascending manner from zero.
/// This index is the same in all program instances since the nodes are ordered by
/// identifier before getting an index assigned
pub struct IBNetwork<T: Ord> {
    node_configs: Vec<IBNodeConfig<T>>,
    indexed_nodes: BTreeMap<T, usize>,
    sender_nodes: Vec<usize>,
    receiver_nodes: Vec<usize>,
}

impl<ID: Ord> IBNetwork<ID> {
    pub fn node<Q>(&self, node_id: &Q) -> Option<&IBNodeConfig<ID>>
    where
        ID: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        let node_idx = self.indexed_nodes.get(node_id)?;
        self.node_configs.get(*node_idx)
    }

    pub fn nodes(&self) -> &[IBNodeConfig<ID>] {
        self.node_configs.as_slice()
    }

    pub fn senders(&self) -> Vec<&IBNodeConfig<ID>> {
        self.sender_nodes
            .iter()
            .map(|&index| &self.node_configs[index])
            .collect()
    }

    pub fn receivers(&self) -> Vec<&IBNodeConfig<ID>> {
        self.receiver_nodes
            .iter()
            .map(|&index| &self.node_configs[index])
            .collect()
    }
}
