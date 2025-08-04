use crate::IbBConnectedEndpoint;
use crate::network::{IbBNetworkConfig, IbBNodeConfig};

pub struct UnsetNetworkConfig;
pub struct SetNetworkConfig(IbBNetworkConfig);

pub struct UnsetNodeSelection;
pub struct SetNodeSelection(IbBNodeConfig);

pub struct IbBSyncEndpointBuilder<NetworkConfigStatus, NodeSelectionStatus> {
    network_config: NetworkConfigStatus,
    node_config: NodeSelectionStatus,
}

impl IbBSyncEndpointBuilder<UnsetNetworkConfig, UnsetNodeSelection> {
    pub fn new() -> Self {
        Self {
            network_config: UnsetNetworkConfig,
            node_config: UnsetNodeSelection,
        }
    }

    pub fn set_network_config(
        self,
        network_config: IbBNetworkConfig,
    ) -> IbBSyncEndpointBuilder<SetNetworkConfig, UnsetNodeSelection> {
        IbBSyncEndpointBuilder {
            network_config: SetNetworkConfig(network_config),
            node_config: UnsetNodeSelection,
        }
    }
}

impl IbBSyncEndpointBuilder<SetNetworkConfig, UnsetNodeSelection> {
    pub fn assign_node_by_rank_id(
        self,
        rank_id: u32,
    ) -> Result<
        IbBSyncEndpointBuilder<SetNetworkConfig, SetNodeSelection>,
        IbBSyncEndpointBuilder<SetNetworkConfig, UnsetNodeSelection>,
    > {
        match self
            .network_config
            .0
            .iter()
            .find(|node_config| node_config.rank_id() == rank_id)
            .map(|node_config| node_config.to_owned())
        {
            Some(node_config) => Ok(IbBSyncEndpointBuilder {
                network_config: self.network_config,
                node_config: SetNodeSelection(node_config),
            }),
            None => Err(IbBSyncEndpointBuilder {
                network_config: self.network_config,
                node_config: self.node_config,
            }),
        }
    }

    pub fn assign_node_by_ut_id<S: AsRef<str>>(
        self,
        ut_id: S,
    ) -> Result<
        IbBSyncEndpointBuilder<SetNetworkConfig, SetNodeSelection>,
        IbBSyncEndpointBuilder<SetNetworkConfig, UnsetNodeSelection>,
    > {
        match self
            .network_config
            .0
            .iter()
            .find(|node_config| node_config.ut_id() == ut_id.as_ref())
            .map(|node_config| node_config.to_owned())
        {
            Some(node_config) => Ok(IbBSyncEndpointBuilder {
                network_config: self.network_config,
                node_config: SetNodeSelection(node_config),
            }),
            None => Err(IbBSyncEndpointBuilder {
                network_config: self.network_config,
                node_config: self.node_config,
            }),
        }
    }
}

impl IbBSyncEndpointBuilder<SetNetworkConfig, SetNodeSelection> {
    pub fn build(self) -> Result<IbBSyncUnconnectedEndpoint, Self> {
        let master_node_config = match self
            .network_config
            .0
            .iter()
            .min_by_key(|node_config| node_config.rank_id())
        {
            Some(master_id) => master_id,
            None => return Err(self),
        };

        if self.node_config.0.rank_id() == master_node_config.rank_id() {
            // Ask for 
        }

        IbBSyncUnconnectedEndpoint {
            network_config: self.network_config.0,
            node_config: self.node_config.0,
        }
    }
}

enum Connections {
    Master(Vec<IbBConnectedEndpoint>),
    Slave(IbBConnectedEndpoint),
}

pub struct IbBSyncUnconnectedEndpoint {
    network_config: IbBNetworkConfig,
    node_config: IbBNodeConfig,
    connections: Connections,
}

impl IbBSyncUnconnectedEndpoint {
    pub fn request_ib_endpoints() {}
}
