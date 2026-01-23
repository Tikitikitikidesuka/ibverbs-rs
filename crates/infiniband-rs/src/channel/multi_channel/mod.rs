pub mod builder;
pub mod mr_ops;
mod polled_ops;
mod scoped;
pub mod unpolled_ops;

use crate::channel::raw_channel::RawChannel;
use crate::ibverbs::protection_domain::ProtectionDomain;
use std::io;

pub struct MultiChannel {
    channels: Box<[RawChannel]>,
    pd: ProtectionDomain,
}

impl MultiChannel {
    fn channel(&mut self, peer: usize) -> io::Result<&mut RawChannel> {
        self.channels.get_mut(peer).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::AddrNotAvailable,
                format!("Peer index {} does not exist", peer),
            )
        })
    }
}
