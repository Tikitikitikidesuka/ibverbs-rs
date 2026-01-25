use crate::channel::multi_channel::MultiChannel;
use crate::channel::multi_channel::builder::PreparedMultiChannel;
use crate::ibverbs::context::Context;
use crate::ibverbs::queue_pair_endpoint::QueuePairEndpoint;
use crate::network::Node;
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
    ) -> io::Result<PreparedNetworkNode> {
        let prepared_multi_channel = MultiChannel::builder()
            .context(context)
            .num_channels(world_size)
            .min_cq_buf_size(min_cq_buf_size)
            .max_send_wrs(max_send_wrs)
            .max_recv_wrs(max_recv_wrs)
            .max_send_sges(max_send_sges)
            .max_recv_sges(max_recv_sges)
            .build()?;

        Ok(PreparedNetworkNode {
            rank,
            world_size,
            prepared_multi_channel,
        })
    }
}

pub struct PreparedNetworkNode {
    rank: usize,
    world_size: usize,
    prepared_multi_channel: PreparedMultiChannel,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LocalEndpoint {
    rank: usize,
    endpoints: Box<[QueuePairEndpoint]>,
}

pub struct RemoteEndpoints(Box<[QueuePairEndpoint]>);

impl PreparedNetworkNode {
    pub fn endpoint(&self) -> LocalEndpoint {
        LocalEndpoint {
            rank: self.rank,
            endpoints: self.prepared_multi_channel.endpoints(),
        }
    }

    pub fn gather_endpoints(
        &self,
        endpoints: impl IntoIterator<Item = LocalEndpoint>,
    ) -> io::Result<RemoteEndpoints> {
        // Temporary initialization tracker
        let mut tmp: Vec<Option<QueuePairEndpoint>> = vec![None; self.world_size];

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
        let in_endpoints: Vec<QueuePairEndpoint> = tmp
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
        let multi_channel = self.prepared_multi_channel.handshake(endpoints.0)?;

        Ok(Node {
            rank: self.rank,
            num_network_nodes: self.world_size,
            multi_channel,
        })
    }
}
