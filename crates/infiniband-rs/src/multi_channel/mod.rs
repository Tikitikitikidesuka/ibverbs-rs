pub mod builder;
pub mod unpolled_ops;
pub mod mr_ops;
mod scoped;
mod polled_ops;

use crate::channel::Channel;
use crate::ibverbs::protection_domain::ProtectionDomain;
use std::io;

pub struct MultiChannel {
    channels: Box<[Channel]>,
    pd: ProtectionDomain,
}

impl MultiChannel {
    fn channel(&mut self, peer: usize) -> io::Result<&mut Channel> {
        self.channels.get_mut(peer).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::AddrNotAvailable,
                format!("Peer index {} does not exist", peer),
            )
        })
    }
}
