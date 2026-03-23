//! Multiple parallel RDMA channels to different peers sharing a single [`ProtectionDomain`].
//!
//! A [`MultiChannel`] holds one [`Channel`] per peer and routes operations by peer index.
//! It supports the same three levels of control as [`Channel`](crate::channel):
//!
//! * **Blocking** — [`scatter_send`](MultiChannel::scatter_send),
//!   [`gather_receive`](MultiChannel::gather_receive), etc.
//! * **Scoped** — [`MultiChannel::scope`] and [`MultiChannel::manual_scope`].
//! * **Unpolled** — `unsafe` variants for manual lifetime management.
//!
//! Work requests are wrapped in peer-aware types ([`PeerSendWorkRequest`](work_request::PeerSendWorkRequest), etc.)
//! that carry the target peer index.

pub mod builder;
pub mod ops;
pub mod polling_scope;
pub mod remote_memory_region;
pub mod work_request;

use crate::channel::Channel;
use crate::ibverbs::error::{IbvError, IbvResult};
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::multi_channel::builder::MultiChannelBuilder;
use crate::multi_channel::builder::multi_channel_builder::SetPd;

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
    /// Returns a [`MultiChannelBuilder`] with this protection domain already set.
    pub fn create_multi_channel(&self) -> MultiChannelBuilder<'_, SetPd> {
        MultiChannel::builder().pd(self)
    }
}
