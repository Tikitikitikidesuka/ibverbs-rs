pub mod builder;
pub mod meta_ops;
pub mod mr_ops;
pub mod polled_ops;
pub mod scoped_ops;
pub mod unpolled_ops;

mod meta_mr;

use crate::channel::raw_channel::RawChannel;
use crate::channel::single_channel::meta_mr::MetaMr;
use crate::ibverbs::protection_domain::ProtectionDomain;

/// This is a single channel with owned protection domain.
/// This allows making safe memory region registration because it cannot be shared
/// with remote except allowed by the struct in which case it should be safe.
/// In the plain Channel, a memory region could be registered to the PD directly and then
/// shared by many different means. That is why registering memory to PD is not safe.
/// When registering memory to a PD, if another peer has remote access to it, it could be
/// freed while still registered and the remote could issue a write unto it. UD.
pub struct SingleChannel {
    channel: RawChannel,
    meta_mr: MetaMr,
    pd: ProtectionDomain,
}
