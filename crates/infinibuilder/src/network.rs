use std::ops::Deref;

#[derive(Debug, Clone)]
pub struct IbBNetworkConfig {
    pub(crate) nodes: Vec<IbBNodeConfig>,
}

impl IbBNetworkConfig {
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    pub fn add_node(mut self, node: IbBNodeConfig) -> Self {
        self.nodes.push(node);
        self
    }

    pub fn nodes(&self) -> &[IbBNodeConfig] {
        self.nodes.as_slice()
    }
}

impl Deref for IbBNetworkConfig {
    type Target = [IbBNodeConfig];

    fn deref(&self) -> &Self::Target {
        self.nodes()
    }
}

#[derive(Debug, Clone)]
pub struct IbBNodeConfig {
    hostname: String,
    ib_device: String,
    rank_id: u32,
    ut_id: String,
}

impl IbBNodeConfig {
    pub fn new<S: Into<String>>(hostname: S, ib_device: S, rank_id: u32, ut_id: S) -> Self {
        Self {
            hostname: hostname.into(),
            ib_device: ib_device.into(),
            rank_id,
            ut_id: ut_id.into(),
        }
    }

    pub fn hostname(&self) -> &str {
        self.hostname.as_str()
    }

    pub fn ib_device(&self) -> &str {
        self.ib_device.as_str()
    }

    pub fn rank_id(&self) -> u32 {
        self.rank_id
    }

    pub fn ut_id(&self) -> &str {
        self.ut_id.as_str()
    }
}
