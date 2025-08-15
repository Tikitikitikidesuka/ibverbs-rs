use ibverbs::Context;
use crate::IbBCheckedStaticNetworkConfig;

pub trait IbBNodeComponentBuilder: Sized {
    type DynamicConfig;
    type Component;

    fn new(
        ib_context: Context,
        static_network_config: &IbBCheckedStaticNetworkConfig,
        rank_id: u32,
    ) -> std::io::Result<Self>;
    fn dynamic_config(&self) -> Self::DynamicConfig;
    fn build(self, dynamic_config: Self::DynamicConfig) -> std::io::Result<Self::Component>;
}
