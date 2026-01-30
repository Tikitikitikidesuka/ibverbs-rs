pub mod builder;
pub mod multi_ops;
pub mod remote_memory_region;
pub mod single_ops;
pub mod work_request;

use crate::channel::Channel;
use crate::ibverbs::protection_domain::ProtectionDomain;
use std::io;

#[derive(Debug)]
pub struct MultiChannel {
    channels: Box<[Channel]>,
    pd: ProtectionDomain,
}

impl MultiChannel {
    pub fn num_channels(&self) -> usize {
        self.channels.len()
    }

    pub fn pd(&self) -> &ProtectionDomain {
        &self.pd
    }

    // Helper for single ops
    pub(super) fn channel(&mut self, peer: usize) -> io::Result<&mut Channel> {
        self.channels.get_mut(peer).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::AddrNotAvailable,
                format!("Peer index {} does not exist", peer),
            )
        })
    }
}
