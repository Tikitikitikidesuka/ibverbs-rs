use crate::channel::multi_channel::MultiChannel;
use crate::channel::raw_channel::RawChannel;
use crate::channel::raw_channel::builder::PreparedChannel;
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
                    .pd(pd.clone())
                    .min_cq_buf_size(min_cq_buf_size)
                    .max_send_wrs(max_send_wrs)
                    .max_recv_wrs(max_recv_wrs)
                    .max_send_sges(max_send_sges)
                    .max_recv_sges(max_recv_sges)
                    .build()
            })
            .collect::<Result<_, _>>()?;
        Ok(PreparedMultiChannel { channels, pd })
    }
}

pub struct PreparedMultiChannel {
    channels: Box<[PreparedChannel]>,
    pd: ProtectionDomain,
}

impl PreparedMultiChannel {
    pub fn endpoints(&self) -> Box<[QueuePairEndpoint]> {
        self.channels
            .iter()
            .map(|channel| channel.endpoint())
            .collect()
    }

    pub fn handshake(self, endpoints: impl AsRef<[QueuePairEndpoint]>) -> io::Result<MultiChannel> {
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
            .map(|(channel, endpoint)| channel.handshake(*endpoint))
            .collect::<Result<_, _>>()?;

        Ok(MultiChannel {
            channels,
            pd: self.pd,
        })
    }
}
