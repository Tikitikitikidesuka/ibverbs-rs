//! A set of RDMA channels to multiple peers sharing a single protection domain.
//!
//! A [`MultiChannel`] holds one [`Channel`] per peer and routes each operation to the
//! correct channel based on the peer index embedded in the work request. All channels
//! share a single [`ProtectionDomain`], so memory regions registered once can be used
//! with any peer without re-registration.
//!
//! # Connection lifecycle
//!
//! Construction mirrors [`Channel`] but establishes a separate queue
//! pair for each peer instead of a single one.
//!
//! 1. **Build** — call [`MultiChannel::builder`] (or
//!    [`ProtectionDomain::create_multi_channel`]) and set the number of peers with
//!    [`num_channels`](MultiChannelBuilder::num_channels). [`build`](MultiChannelBuilder::build)
//!    returns a [`PreparedMultiChannel`].
//! 2. **Handshake** — collect the local [`endpoints`](PreparedMultiChannel::endpoints),
//!    exchange them with every peer out-of-band, then call
//!    [`PreparedMultiChannel::handshake`] with the full list of remote endpoints to
//!    obtain the connected [`MultiChannel`].
//!
//! # Peer-indexed work requests
//!
//! Every operation takes a peer-aware wrapper that pairs a standard work request with
//! a target (or source) peer index:
//!
//! * [`PeerSendWorkRequest`] / [`PeerReceiveWorkRequest`] — two-sided messaging.
//! * [`PeerWriteWorkRequest`] / [`PeerReadWorkRequest`] — one-sided RDMA.
//!
//! # Posting operations
//!
//! The same three control levels as [`channel`](crate::channel) are available, extended
//! to operate over multiple peers at once:
//!
//! * **Blocking** — [`scatter_send`](MultiChannel::scatter_send),
//!   [`scatter_write`](MultiChannel::scatter_write),
//!   [`gather_receive`](MultiChannel::gather_receive),
//!   [`gather_read`](MultiChannel::gather_read) post an iterator of per-peer work
//!   requests and block until all complete.
//!   [`multicast_send`](MultiChannel::multicast_send) fans the same send out to an
//!   arbitrary set of peers.
//! * **Scoped** — [`MultiChannel::scope`] and [`MultiChannel::manual_scope`] open a
//!   [`PollingScope`](crate::channel::PollingScope) whose `post_scatter_*` /
//!   `post_gather_*` / `post_multicast_send` methods return
//!   [`ScopedPendingWork`](crate::channel::ScopedPendingWork) handles for fine-grained
//!   polling. All outstanding work is automatically polled when the scope exits.
//! * **Unpolled** — `unsafe` `scatter_*_unpolled` / `gather_*_unpolled` variants
//!   return raw [`PendingWork`](crate::channel::PendingWork) handles for maximum
//!   control. Prefer the scoped API unless you need direct access to these primitives.
//!
//! [`ProtectionDomain`]: crate::ibverbs::protection_domain::ProtectionDomain

mod builder;
mod ops;
mod polling_scope;
mod remote_memory_region;
mod work_request;

#[doc(hidden)]
pub use builder::multi_channel_builder::{
    Empty, SetAccess, SetAckTimeout, SetMaxAckRetries, SetMaxRecvSge, SetMaxRecvWr,
    SetMaxRnrRetries, SetMaxSendSge, SetMaxSendWr, SetMinCqEntries, SetMinRnrTimer, SetMtu,
    SetNumChannels, SetPd, SetRecvPsn, SetSendPsn,
};
pub use builder::{MultiChannelBuilder, PreparedMultiChannel};
pub use remote_memory_region::PeerRemoteMemoryRegion;
pub use work_request::{
    PeerReadWorkRequest, PeerReceiveWorkRequest, PeerSendWorkRequest, PeerWriteWorkRequest,
};

use crate::channel::Channel;
use crate::ibverbs::error::{IbvError, IbvResult};
use crate::ibverbs::protection_domain::ProtectionDomain;

/// A set of [`Channel`]s to different peers, sharing a single [`ProtectionDomain`].
///
/// Each peer is identified by its index. Operations are routed to the correct channel
/// based on the peer index in the work request.
///
/// Use [`ProtectionDomain::create_multi_channel`] or [`MultiChannel::builder`] to construct one.
#[derive(Debug)]
pub struct MultiChannel {
    channels: Box<[Channel]>,
    pd: ProtectionDomain,
}

impl MultiChannel {
    /// Returns the number of peer channels.
    pub fn num_channels(&self) -> usize {
        self.channels.len()
    }

    /// Returns a reference to the shared [`ProtectionDomain`].
    pub fn pd(&self) -> &ProtectionDomain {
        &self.pd
    }

    pub(crate) fn channel(&mut self, peer: usize) -> IbvResult<&mut Channel> {
        self.channels
            .get_mut(peer)
            .ok_or_else(|| IbvError::NotFound(format!("Peer {peer} not found")))
    }
}

impl ProtectionDomain {
    /// Returns a builder with this protection domain already set.
    pub fn create_multi_channel(&self) -> MultiChannelBuilder<'_, SetPd> {
        MultiChannel::builder().pd(self)
    }
}
