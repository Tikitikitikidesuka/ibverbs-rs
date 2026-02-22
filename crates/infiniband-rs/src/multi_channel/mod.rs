pub mod builder;
pub mod ops;
pub mod polling_scope;
pub mod remote_memory_region;
pub mod work_request;

use crate::channel::Channel;
use crate::ibverbs::error::{IbvError, IbvResult};
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

    pub fn channel(&mut self, peer: usize) -> IbvResult<&mut Channel> {
        self.channels
            .get_mut(peer)
            .ok_or_else(|| IbvError::NotFound(format!("Peer {peer} not found")))
    }
}

impl ProtectionDomain {
    pub fn create_multi_channel(&self) -> MultiChannelBuilder<'_, SetPd> {
        MultiChannel::builder().pd(self)
    }
}
