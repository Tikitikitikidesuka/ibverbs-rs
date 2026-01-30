use crate::ibverbs::access_config::AccessFlags;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::ibverbs::queue_pair::builder::QueuePairEndpoint;
use crate::ibverbs::queue_pair::config::{
    AckTimeout, MaxAckRetries, MaxRnrRetries, MaximumTransferUnit, MinRnrTimer,
    PacketSequenceNumber,
};
use crate::multi_channel::MultiChannel;
use crate::multi_channel::builder::PreparedMultiChannel;
use crate::multi_channel::remote_memory_region::PeerRemoteMemoryRegion;
use crate::network::Node;
use crate::network::barrier::{CentralizedBarrier, PreparedCentralizedBarrier};
use bon::bon;
use serde::{Deserialize, Serialize};
use std::io;

#[bon]
impl Node {
    #[builder(state_mod(vis = "pub(crate)"))]
    pub fn builder(
        rank: usize,
        world_size: usize,
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
    ) -> io::Result<PreparedNode> {
        let multi_channel = MultiChannel::builder()
            .num_channels(world_size)
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
            .build()?;
        let barrier = CentralizedBarrier::new(pd, rank, world_size)?;

        Ok(PreparedNode {
            rank,
            world_size,
            multi_channel,
            barrier,
        })
    }
}

pub struct PreparedNode {
    rank: usize,
    world_size: usize,
    multi_channel: PreparedMultiChannel,
    barrier: PreparedCentralizedBarrier,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct NetworkChannelEndpoint {
    pub(crate) single_channel_endpoint: QueuePairEndpoint,
    pub(crate) barrier_mr_remote: PeerRemoteMemoryRegion,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LocalEndpoint {
    rank: usize,
    endpoints: Box<[NetworkChannelEndpoint]>,
}

pub struct RemoteEndpoints(Box<[NetworkChannelEndpoint]>);

impl PreparedNode {
    pub fn endpoint(&self) -> LocalEndpoint {
        LocalEndpoint {
            rank: self.rank,
            endpoints: self
                .multi_channel
                .endpoints()
                .into_iter()
                .map(|single_channel_endpoint| NetworkChannelEndpoint {
                    single_channel_endpoint,
                    barrier_mr_remote: self.barrier.remote_mr(),
                })
                .collect(),
        }
    }

    pub fn gather_endpoints(
        &self,
        endpoints: impl IntoIterator<Item = LocalEndpoint>,
    ) -> io::Result<RemoteEndpoints> {
        // Temporary initialization tracker
        let mut tmp: Vec<Option<NetworkChannelEndpoint>> = vec![None; self.world_size];

        for endpoint in endpoints {
            // Check rank bounds
            let in_slot = tmp.get_mut(endpoint.rank).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "Input endpoint rank {} out of bounds (0..{})",
                        endpoint.rank, self.world_size
                    ),
                )
            })?;

            // Detect duplicate ranks
            if in_slot.is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Duplicate endpoint for rank {}", endpoint.rank),
                ));
            }

            // Check that the endpoint has a rechannel for our rank
            let qp_endpoint = endpoint.endpoints.get(self.rank).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "Input endpoint for rank {} missing rechannel for local rank {}",
                        endpoint.rank, self.rank
                    ),
                )
            })?;

            // Fill the temporary slot
            *in_slot = Some(*qp_endpoint);
        }

        // Convert Option<Vec<_>> -> Vec<_> in one go, validating all slots are filled
        let in_endpoints: Vec<NetworkChannelEndpoint> = tmp
            .into_iter()
            .enumerate()
            .map(|(i, opt)| {
                opt.ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Missing endpoint from rank {}", i),
                    )
                })
            })
            .collect::<Result<_, _>>()?;

        Ok(RemoteEndpoints(in_endpoints.into_boxed_slice()))
    }

    pub fn handshake(self, endpoints: RemoteEndpoints) -> io::Result<Node> {
        let multi_channel = self
            .multi_channel
            .handshake(endpoints.0.iter().map(|e| e.single_channel_endpoint))?;
        let barrier = self
            .barrier
            .link_remote(endpoints.0.iter().map(|e| e.barrier_mr_remote).collect());

        Ok(Node {
            rank: self.rank,
            world_size: self.world_size,
            multi_channel,
            barrier,
        })
    }
}
