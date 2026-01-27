use crate::channel::meta_mr::{MetaChannelEndpoint, MetaMr, PreparedMetaMr};
use crate::channel::raw_channel::RawChannel;
use crate::channel::raw_channel::builder::PreparedChannel;
use crate::channel::single_channel::SingleChannel;
use crate::ibverbs::context::Context;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::ibverbs::queue_pair_endpoint::QueuePairEndpoint;
use crate::ibverbs::remote_memory_region::RemoteMemoryRegion;
use bon::bon;
use serde::{Deserialize, Serialize};
use std::io;

#[bon]
impl SingleChannel {
    #[builder]
    pub fn builder(
        context: &Context,
        #[builder(default = 32)] min_cq_buf_size: u32,
        #[builder(default = 32)] max_send_wrs: u32,
        #[builder(default = 32)] max_recv_wrs: u32,
        #[builder(default = 32)] max_send_sges: u32,
        #[builder(default = 32)] max_recv_sges: u32,
    ) -> io::Result<PreparedSingleChannel> {
        let pd = context.allocate_pd()?;
        let channel = RawChannel::builder()
            .pd(&pd)
            .min_cq_buf_size(min_cq_buf_size)
            .max_send_wrs(max_send_wrs)
            .max_recv_wrs(max_recv_wrs)
            .max_send_sges(max_send_sges)
            .max_recv_sges(max_recv_sges)
            .build()?;
        let meta_mr = MetaMr::new(&pd)?;
        Ok(PreparedSingleChannel {
            channel,
            meta_mr,
            pd,
        })
    }
}

pub struct PreparedSingleChannel {
    channel: PreparedChannel,
    meta_mr: PreparedMetaMr,
    pd: ProtectionDomain,
}

impl PreparedSingleChannel {
    pub fn endpoint(&self) -> MetaChannelEndpoint {
        MetaChannelEndpoint {
            channel_endpoint: self.channel.endpoint(),
            meta_mr_remote: self.meta_mr.remote(),
        }
    }

    pub fn handshake(self, endpoint: MetaChannelEndpoint) -> io::Result<SingleChannel> {
        let channel = self.channel.handshake(endpoint.channel_endpoint)?;
        let meta_mr = self.meta_mr.link_remote(endpoint.meta_mr_remote);

        Ok(SingleChannel {
            channel,
            meta_mr,
            pd: self.pd,
        })
    }
}
