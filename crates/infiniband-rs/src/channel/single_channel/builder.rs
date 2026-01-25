use crate::channel::raw_channel::RawChannel;
use crate::channel::raw_channel::builder::PreparedChannel;
use crate::channel::single_channel::SingleChannel;
use crate::ibverbs::context::Context;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::ibverbs::queue_pair_endpoint::QueuePairEndpoint;
use bon::bon;
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
            .pd(pd.clone())
            .min_cq_buf_size(min_cq_buf_size)
            .max_send_wrs(max_send_wrs)
            .max_recv_wrs(max_recv_wrs)
            .max_send_sges(max_send_sges)
            .max_recv_sges(max_recv_sges)
            .build()?;
        Ok(PreparedSingleChannel { channel, pd })
    }
}

pub struct PreparedSingleChannel {
    channel: PreparedChannel,
    pd: ProtectionDomain,
}

impl PreparedSingleChannel {
    pub fn endpoint(&self) -> QueuePairEndpoint {
        self.channel.endpoint()
    }

    pub fn handshake(self, endpoint: QueuePairEndpoint) -> io::Result<SingleChannel> {
        let channel = self.channel.handshake(endpoint)?;
        Ok(SingleChannel {
            channel,
            pd: self.pd,
        })
    }
}
