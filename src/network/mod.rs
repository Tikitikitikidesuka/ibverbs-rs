//! Distributed RDMA network node — TCP-based endpoint exchange, rank/world-size coordination, and barrier synchronization (linear, binary tree, and dissemination).
//!
//! A [`Node`] combines a [`MultiChannel`] with a rank, a world size, and a
//! [`Barrier`] to form a complete building block for distributed RDMA programs.
//! It exposes the full [`multi_channel`](crate::multi_channel) operation API
//! (scatter/gather sends, writes, reads, multicast) and adds barrier synchronization
//! for coordinating execution across all nodes in the network.
//!
//! # Connection lifecycle
//!
//! Connecting a set of nodes requires exchanging endpoint information between every
//! pair of participants. The [`tcp_exchanger`](Exchanger) utility performs this
//! out-of-band exchange over TCP, driven by a shared [`RawNetworkConfig`] that
//! describes the address and port of each node.
//!
//! 1. **Build** — call [`Node::builder`] (or [`ProtectionDomain::create_node`]) and
//!    set at minimum `rank`, `world_size`, and `pd`. An optional
//!    [`BarrierAlgorithm`] can be chosen; the default is
//!    [`BinaryTree`](BarrierAlgorithm::BinaryTree).
//! 2. **Exchange endpoints** — call [`Node::endpoint`](PreparedNode::endpoint) to
//!    obtain the local [`LocalEndpoint`], then use
//!    [`Exchanger::await_exchange_all`] to distribute it to all peers and collect
//!    theirs. Pass the result through [`Node::gather_endpoints`](PreparedNode::gather_endpoints)
//!    to produce [`RemoteEndpoints`] in the format expected by the handshake.
//! 3. **Handshake** — call [`PreparedNode::handshake`] with the remote endpoints to
//!    bring up all queue pairs and obtain the ready-to-use [`Node`].
//!
//! # Operations
//!
//! All [`MultiChannel`] operations are forwarded directly on [`Node`]:
//! [`scatter_send`](Node::scatter_send), [`gather_receive`](Node::gather_receive),
//! [`scatter_write`](Node::scatter_write), [`gather_read`](Node::gather_read), and
//! [`multicast_send`](Node::multicast_send), along with their scoped and unpolled
//! variants via [`Node::scope`] and [`Node::manual_scope`].
//!
//! # Barrier synchronization
//!
//! [`Node::barrier`] blocks until every node in the supplied peer list has called
//! barrier, or until the timeout expires. The peer list may be any subset of the
//! world, allowing partial barriers across subgroups.
//! [`Node::barrier_unchecked`] skips peer-list validation for hot paths.
//!
//! The barrier algorithm is selected at build time via [`BarrierAlgorithm`]:
//!
//! * [`Centralized`](BarrierAlgorithm::Centralized) — the lowest-ranked participant
//!   acts as coordinator; simple but does not scale well.
//! * [`Dissemination`](BarrierAlgorithm::Dissemination) — pairwise exchange at
//!   exponential distances; no designated leader, scales well.
//! * [`BinaryTree`](BarrierAlgorithm::BinaryTree) — tree-based reduce and broadcast;
//!   a balanced alternative to dissemination.
//!
//! # Network configuration
//!
//! [`RawNetworkConfig`] is a serializable description of the cluster (one
//! [`NodeConfig`] per rank, each with an IP address and port) that can be loaded from
//! JSON. [`RawNetworkConfig::build`] validates it and produces a [`NetworkConfig`]
//! ready for use with [`Exchanger`].
//!
//! [`MultiChannel`]: crate::multi_channel::MultiChannel

mod barrier;
mod builder;
mod config;
mod ops;
mod polling_scope;
mod tcp_exchanger;

pub use barrier::{Barrier, BarrierAlgorithm, BarrierError, PreparedBarrier};
#[doc(hidden)]
pub use builder::node_builder::{
    Empty, SetAccess, SetAckTimeout, SetBarrier, SetMaxAckRetries, SetMaxRecvSge, SetMaxRecvWr,
    SetMaxRnrRetries, SetMaxSendSge, SetMaxSendWr, SetMinCqEntries, SetMinRnrTimer, SetMtu, SetPd,
    SetRank, SetRecvPsn, SetSendPsn, SetWorldSize,
};
pub use builder::{
    LocalEndpoint, NetworkChannelEndpoint, NodeBuilder, PreparedNode, RemoteEndpoints,
};
pub use config::{NetworkConfig, NetworkConfigError, NodeConfig, RawNetworkConfig};
pub use tcp_exchanger::{ExchangeConfig, ExchangeError, Exchanger};

use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::multi_channel::MultiChannel;

/// A ranked RDMA network node with barrier synchronization.
///
/// Wraps a [`MultiChannel`] with a rank, world size, and a [`Barrier`] for
/// collective synchronization across all nodes in the network.
#[derive(Debug)]
pub struct Node {
    rank: usize,
    world_size: usize,
    multi_channel: MultiChannel,
    barrier: Barrier,
}

impl Node {
    /// Returns the protection domain this node belongs to.
    pub fn pd(&self) -> &ProtectionDomain {
        self.multi_channel.pd()
    }

    /// Returns the total number of nodes in the network.
    pub fn world_size(&self) -> usize {
        self.world_size
    }

    /// Returns this node's rank (index) in the network.
    pub fn rank(&self) -> usize {
        self.rank
    }
}

impl ProtectionDomain {
    /// Creates a builder under this protection domain.
    pub fn create_node(&self) -> NodeBuilder<'_, SetPd> {
        Node::builder().pd(self)
    }
}
