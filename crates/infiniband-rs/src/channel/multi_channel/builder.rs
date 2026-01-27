use crate::channel::meta_mr::{MetaMr, PreparedMetaMr};
use crate::channel::multi_channel::MultiChannel;
use crate::channel::raw_channel::RawChannel;
use crate::channel::raw_channel::builder::PreparedChannel;
use crate::channel::single_channel::builder::SingleChannelEndpoint;
use crate::ibverbs::context::Context;
use crate::ibverbs::protection_domain::ProtectionDomain;
use bon::bon;
use std::io;

#[bon]
impl MultiChannel {
    #[builder]
    pub fn builder(
        context: &Context,
        num_channels: usize,
        #[builder(default = 32)] min_cq_buf_size: u32,
        #[builder(default = 32)] max_send_wrs: u32,
        #[builder(default = 32)] max_recv_wrs: u32,
        #[builder(default = 32)] max_send_sges: u32,
        #[builder(default = 32)] max_recv_sges: u32,
    ) -> io::Result<PreparedMultiChannel> {
        let pd = context.allocate_pd()?;
        let channels = (0..num_channels)
            .into_iter()
            .map(|_| {
                RawChannel::builder()
                    .pd(&pd)
                    .min_cq_buf_size(min_cq_buf_size)
                    .max_send_wrs(max_send_wrs)
                    .max_recv_wrs(max_recv_wrs)
                    .max_send_sges(max_send_sges)
                    .max_recv_sges(max_recv_sges)
                    .build()
            })
            .collect::<io::Result<_>>()?;
        let meta_mrs = (0..num_channels)
            .into_iter()
            .map(|_| MetaMr::new(&pd))
            .collect::<io::Result<_>>()?;
        Ok(PreparedMultiChannel {
            channels,
            meta_mrs,
            pd,
        })
    }
}

pub struct PreparedMultiChannel {
    channels: Box<[PreparedChannel]>,
    meta_mrs: Box<[PreparedMetaMr]>,
    pd: ProtectionDomain,
}

impl PreparedMultiChannel {
    pub fn endpoints(&self) -> Box<[SingleChannelEndpoint]> {
        self.channels
            .iter()
            .zip(self.meta_mrs.iter())
            .map(|(channel, meta_mr)| SingleChannelEndpoint {
                channel_endpoint: channel.endpoint(),
                meta_mr_remote: meta_mr.remote(),
            })
            .collect()
    }

    pub fn handshake(
        self,
        endpoints: impl AsRef<[SingleChannelEndpoint]>,
    ) -> io::Result<MultiChannel> {
        if self.channels.len() != endpoints.as_ref().len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "expected {} endpoints, got {}",
                    self.channels.len(),
                    endpoints.as_ref().len()
                ),
            ));
        }

        let channels = self
            .channels
            .into_iter()
            .zip(endpoints.as_ref())
            .map(|(channel, endpoint)| channel.handshake(endpoint.channel_endpoint))
            .collect::<io::Result<_>>()?;
        let meta_mrs = self
            .meta_mrs
            .into_iter()
            .zip(endpoints.as_ref())
            .map(|(meta_mr, endpoint)| meta_mr.link_remote(endpoint.meta_mr_remote))
            .collect();

        Ok(MultiChannel {
            channels,
            meta_mrs,
            pd: self.pd,
        })
    }
}
