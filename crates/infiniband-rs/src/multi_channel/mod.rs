pub mod builder;
pub mod multi_ops;
pub mod remote_memory_region;
pub mod single_ops;
pub mod work_request;

use crate::channel::Channel;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::multi_channel::builder::MultiChannelBuilder;
use crate::multi_channel::builder::multi_channel_builder::SetPd;

#[derive(Debug)]
pub struct MultiChannel {
    channels: Box<[Channel]>,
    pd: ProtectionDomain,
}

impl MultiChannel {
    pub fn num_channels(&self) -> usize {
        self.channels.len()
    }

    pub fn pd(&self) -> &ProtectionDomain {
        &self.pd
    }
}

impl ProtectionDomain {
    pub fn create_multi_channel(&self) -> MultiChannelBuilder<'_, SetPd> {
        MultiChannel::builder().pd(self)
    }
}
