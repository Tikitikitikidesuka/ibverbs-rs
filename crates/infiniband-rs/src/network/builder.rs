use crate::channel::multi_channel::MultiChannel;
use crate::channel::multi_channel::builder::PreparedMultiChannel;
use crate::channel::multi_channel::rank_remote_memory_region::RankRemoteMemoryRegion;
use crate::channel::single_channel::builder::SingleChannelEndpoint;
use crate::ibverbs::context::Context;
use crate::network::Node;
use crate::network::barrier::{CentralizedBarrier, PreparedCentralizedBarrier};
use bon::bon;
use serde::{Deserialize, Serialize};
use std::io;

#[bon]
impl Node {
    #[builder]
    pub fn builder(
        context: &Context,
        rank: usize,
        world_size: usize,
        #[builder(default = 32)] min_cq_buf_size: u32,
        #[builder(default = 32)] max_send_wrs: u32,
        #[builder(default = 32)] max_recv_wrs: u32,
        #[builder(default = 32)] max_send_sges: u32,
        #[builder(default = 32)] max_recv_sges: u32,
    ) -> io::Result<PreparedNode> {
        let multi_channel = MultiChannel::builder()
            .context(context)
            .num_channels(world_size)
            .min_cq_buf_size(min_cq_buf_size)
            .max_send_wrs(max_send_wrs)
            .max_recv_wrs(max_recv_wrs)
            .max_send_sges(max_send_sges)
            .max_recv_sges(max_recv_sges)
            .build()?;
        let barrier = CentralizedBarrier::new(&multi_channel.pd, rank, world_size)?;

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
    pub(crate) single_channel_endpoint: SingleChannelEndpoint,
    pub(crate) barrier_mr_remote: RankRemoteMemoryRegion,
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

            // Check that the endpoint has a channel for our rank
            let qp_endpoint = endpoint.endpoints.get(self.rank).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "Input endpoint for rank {} missing channel for local rank {}",
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
