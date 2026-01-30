use crate::channel::meta_mr::{MetaMr, PreparedMetaMr};
use crate::channel::multi_channel::MultiChannel;
use crate::channel::raw_channel::RawChannel;
use crate::channel::raw_channel::builder::PreparedChannel;
use crate::channel::single_channel::builder::SingleChannelEndpoint;
use crate::ibverbs::context::Context;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::ibverbs::queue_pair_endpoint::QueuePairEndpoint;
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
    pub(crate) pd: ProtectionDomain,
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

    pub fn handshake<I>(self, endpoints: I) -> io::Result<MultiChannel>
    where
        I: IntoIterator<Item = SingleChannelEndpoint>,
        I::IntoIter: ExactSizeIterator,
    {
        let endpoints = endpoints.into_iter();
        if self.channels.len() != endpoints.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "expected {} endpoints, got {}",
                    self.channels.len(),
                    endpoints.len()
                ),
            ));
        }

        let mut channels = Vec::with_capacity(self.channels.len());
        let mut meta_mrs = Vec::with_capacity(self.meta_mrs.len());

        for ((channel, meta_mr), endpoint) in
            self.channels.into_iter().zip(self.meta_mrs).zip(endpoints)
        {
            channels.push(channel.handshake(endpoint.channel_endpoint)?);
            meta_mrs.push(meta_mr.link_remote(endpoint.meta_mr_remote));
        }

        Ok(MultiChannel {
            channels: channels.into_boxed_slice(),
            meta_mrs: meta_mrs.into_boxed_slice(),
            pd: self.pd,
        })
    }
}
