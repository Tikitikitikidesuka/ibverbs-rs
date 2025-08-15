use crate::IbBStaticNodeConfig;
use ibverbs::QueuePairEndpoint;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::Deref;
use crate::network_config::dynamic_config::IbBDynamicNodeConfig;

// Nodes are guaranteed to be in the same order on every instance of the same network
#[derive(Debug, Clone)]
pub struct IbBReadyNetworkConfig {
    pub(crate) node_config_map: HashMap<u32, IbBReadyNodeConfig>,
    pub(crate) rank_ids: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IbBReadyNodeConfig {
    pub(crate) node_config: IbBStaticNodeConfig,
    pub(crate) dynamic_config: IbBDynamicNodeConfig,
}

impl Deref for IbBReadyNetworkConfig {
    type Target = HashMap<u32, IbBReadyNodeConfig>;

    fn deref(&self) -> &Self::Target {
        &self.node_config_map
    }
}

impl IbBReadyNetworkConfig {
    pub(crate) fn new(
        node_config_map: HashMap<u32, IbBReadyNodeConfig>,
        rank_ids: Vec<u32>,
    ) -> Self {
        let mut sorted_rank_ids = rank_ids;
        sorted_rank_ids.sort();

        Self {
            node_config_map,
            rank_ids: sorted_rank_ids,
        }
    }

    // Guaranteed to be sorted by sorting the vec on instantiation
    pub fn iter(&self) -> IbBReadyNetworkIter {
        IbBReadyNetworkIter {
            rank_ids: &self.rank_ids,
            node_map: &self.node_config_map,
            index: 0,
        }
    }
}

pub struct IbBReadyNetworkIter<'a> {
    rank_ids: &'a Vec<u32>,
    node_map: &'a HashMap<u32, IbBReadyNodeConfig>,
    index: usize,
}

impl<'a> Iterator for IbBReadyNetworkIter<'a> {
    type Item = &'a IbBReadyNodeConfig;

    fn next(&mut self) -> Option<Self::Item> {
        let res = self.node_map.get(self.rank_ids.get(self.index)?);
        self.index += 1;
        res
    }
}

impl Deref for IbBReadyNodeConfig {
    type Target = IbBStaticNodeConfig;

    fn deref(&self) -> &Self::Target {
        &self.node_config
    }
}

impl IbBReadyNodeConfig {
    pub fn dynamic_config(&self) -> &IbBDynamicNodeConfig {
        &self.dynamic_config
    }
}
