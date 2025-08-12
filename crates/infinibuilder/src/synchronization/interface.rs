pub trait IbBNodeSyncBuilder: Sized {
    type StaticConfig;
    type DynamicConfig;
    type IbBNodeSync: IbBNodeSync;

    fn new(static_config: Self::StaticConfig) -> std::io::Result<Self>;
    fn dynamic_config(&self) -> std::io::Result<Self::DynamicConfig>;
    fn build(self, dynamic_config: Self::DynamicConfig) -> std::io::Result<Self::IbBNodeSync>;
}

pub trait IbBNodeSync {
    fn wait_barrier(&mut self) -> std::io::Result<()>;
}
