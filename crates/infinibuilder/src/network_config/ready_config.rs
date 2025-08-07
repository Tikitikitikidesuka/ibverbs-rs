use crate::IbBStaticNodeConfig;
use ibverbs::QueuePairEndpoint;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::Deref;

#[derive(Debug, Clone)]
pub struct IbBReadyNetworkConfig {
    pub(crate) node_config_map: HashMap<u32, IbBReadyNodeConfig>,
    pub(crate) rank_ids: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IbBReadyNodeConfig {
    pub(crate) node_config: IbBStaticNodeConfig,
    pub(crate) qp_endpoint: QueuePairEndpoint,
}

impl Deref for IbBReadyNetworkConfig {
    type Target = HashMap<u32, IbBReadyNodeConfig>;

    fn deref(&self) -> &Self::Target {
        &self.node_config_map
    }
}

impl IbBReadyNetworkConfig {
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
    pub fn qp_endpoint(&self) -> QueuePairEndpoint {
        self.qp_endpoint
    }
}
