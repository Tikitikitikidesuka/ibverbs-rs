use crate::channel::Channel;
use crate::channel::builder::PreparedChannel;
use crate::ibverbs::access_config::AccessFlags;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::ibverbs::queue_pair::builder::QueuePairEndpoint;
use crate::ibverbs::queue_pair::config::{
    AckTimeout, MaxAckRetries, MaxRnrRetries, MaximumTransferUnit, MinRnrTimer,
    PacketSequenceNumber,
};
use crate::multi_channel::MultiChannel;
use bon::bon;
use std::io;

#[bon]
impl MultiChannel {
    #[builder(state_mod(vis = "pub(crate)"))]
    pub fn builder(
        num_channels: usize,
        pd: &ProtectionDomain,
        #[builder(default =
            AccessFlags::new()
                .with_local_write()
                .with_remote_read()
                .with_remote_write()
        )]
        access: AccessFlags,
        #[builder(default = 32)] min_cq_entries: u32,
        #[builder(default = 16)] max_send_wr: u32,
        #[builder(default = 16)] max_recv_wr: u32,
        #[builder(default = 16)] max_send_sge: u32,
        #[builder(default = 16)] max_recv_sge: u32,
        #[builder(default)] max_rnr_retries: MaxRnrRetries,
        #[builder(default)] max_ack_retries: MaxAckRetries,
        #[builder(default)] min_rnr_timer: MinRnrTimer,
        #[builder(default)] ack_timeout: AckTimeout,
        #[builder(default)] mtu: MaximumTransferUnit,
        #[builder(default)] send_psn: PacketSequenceNumber,
        #[builder(default)] recv_psn: PacketSequenceNumber,
    ) -> io::Result<PreparedMultiChannel> {
        let channels = (0..num_channels)
            .into_iter()
            .map(|_| {
                Channel::builder()
                    .pd(pd)
                    .min_cq_entries(min_cq_entries)
                    .access(access)
                    .max_send_wr(max_send_wr)
                    .max_recv_wr(max_recv_wr)
                    .max_send_sge(max_send_sge)
                    .max_recv_sge(max_recv_sge)
                    .max_rnr_retries(max_rnr_retries)
                    .max_ack_retries(max_ack_retries)
                    .min_rnr_timer(min_rnr_timer)
                    .ack_timeout(ack_timeout)
                    .mtu(mtu)
                    .send_psn(send_psn)
                    .recv_psn(recv_psn)
                    .build()
            })
            .collect::<Result<_, _>>()?;

        Ok(PreparedMultiChannel {
            channels,
            pd: pd.clone(),
        })
    }
}

pub struct PreparedMultiChannel {
    channels: Box<[PreparedChannel]>,
    pd: ProtectionDomain,
}

impl PreparedMultiChannel {
    pub fn endpoints(&self) -> Box<[QueuePairEndpoint]> {
        self.channels.iter().map(|c| c.endpoint()).collect()
    }

    pub fn handshake<I>(self, endpoints: I) -> io::Result<MultiChannel>
    where
        I: IntoIterator<Item = QueuePairEndpoint>,
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

        let channels = self
            .channels
            .into_iter()
            .zip(endpoints)
            .map(|(channel, endpoint)| channel.handshake(endpoint))
            .collect::<Result<_, _>>()?;

        Ok(MultiChannel {
            channels,
            pd: self.pd,
        })
    }
}
