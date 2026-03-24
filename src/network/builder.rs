use crate::ibverbs::access_config::AccessFlags;
use crate::ibverbs::error::IbvResult;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::ibverbs::queue_pair::builder::QueuePairEndpoint;
use crate::ibverbs::queue_pair::config::*;
use crate::multi_channel::MultiChannel;
use crate::multi_channel::{PeerRemoteMemoryRegion, PreparedMultiChannel};
use crate::network::Node;
use crate::network::barrier::{BarrierAlgorithm, PreparedBarrier};
use bon::bon;
use serde::{Deserialize, Serialize};
use std::io;

#[bon]
impl Node {
    #[builder(state_mod(vis = "pub"))]
    pub fn builder(
        rank: usize,
        world_size: usize,
        pd: &ProtectionDomain,
        #[builder(default = BarrierAlgorithm::BinaryTree)] barrier: BarrierAlgorithm,
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
    ) -> IbvResult<PreparedNode> {
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
        let barrier = barrier.instance(pd, rank, world_size)?;

        Ok(PreparedNode {
            rank,
            world_size,
            multi_channel,
            barrier,
        })
    }
}

/// A [`Node`] that has been configured but not yet connected to its peers.
///
/// Created by [`Node::builder`]. Call [`endpoint`](Self::endpoint) to obtain the local
/// connection information, exchange endpoints with all peers, then call
/// [`gather_endpoints`](Self::gather_endpoints) followed by [`handshake`](Self::handshake)
/// to finish the connections.
pub struct PreparedNode {
    rank: usize,
    world_size: usize,
    multi_channel: PreparedMultiChannel,
    barrier: PreparedBarrier,
}

/// The per-peer endpoint information exchanged during setup, containing both
/// the queue pair endpoint and the barrier memory region handle.
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct NetworkChannelEndpoint {
    pub(crate) single_channel_endpoint: QueuePairEndpoint,
    pub(crate) barrier_mr_remote: PeerRemoteMemoryRegion,
}

/// This node's endpoint information, ready to be sent to all peers.
#[derive(Clone, Serialize, Deserialize)]
pub struct LocalEndpoint {
    rank: usize,
    endpoints: Box<[NetworkChannelEndpoint]>,
}

/// Validated collection of remote endpoints, one per peer. Produced by
/// [`PreparedNode::gather_endpoints`] and consumed by [`PreparedNode::handshake`].
///
/// Contains one [`NetworkChannelEndpoint`] per rank, indexed in rank order.
pub struct RemoteEndpoints(Box<[NetworkChannelEndpoint]>);

impl PreparedNode {
    /// Returns this node's local endpoint information to be exchanged with all peers.
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

    /// Collects and validates endpoints received from all peers.
    ///
    /// Each peer's [`LocalEndpoint`] must appear exactly once. Returns an error if
    /// any rank is out of bounds, duplicated, or missing.
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

    /// Connects all channels and the barrier, returning a ready-to-use [`Node`].
    pub fn handshake(self, endpoints: RemoteEndpoints) -> IbvResult<Node> {
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
