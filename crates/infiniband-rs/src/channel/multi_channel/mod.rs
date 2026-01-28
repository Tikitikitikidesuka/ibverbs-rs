pub mod builder;
pub mod mr_ops;
pub mod multi_ops;
pub mod rank_remote_memory_region;
pub mod rank_work_request;
pub mod single_ops;

use crate::channel::meta_mr::MetaMr;
use crate::channel::raw_channel::RawChannel;
use crate::ibverbs::protection_domain::ProtectionDomain;
use std::io;

pub struct MultiChannel {
    channels: Box<[RawChannel]>,
    meta_mrs: Box<[MetaMr]>,
    pd: ProtectionDomain,
}

impl MultiChannel {
    pub fn num_channels(&self) -> usize {
        self.channels.len()
    }

    fn channel(&mut self, peer: usize) -> io::Result<&mut RawChannel> {
        self.channels.get_mut(peer).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::AddrNotAvailable,
                format!("Peer index {} does not exist", peer),
            )
        })
    }

    fn meta_mr(&mut self, peer: usize) -> io::Result<&mut MetaMr> {
        self.meta_mrs.get_mut(peer).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::AddrNotAvailable,
                format!("Peer index {} does not exist", peer),
            )
        })
    }

    fn meta_channel(&mut self, peer: usize) -> io::Result<(&mut RawChannel, &mut MetaMr)> {
        let Self {
            channels, meta_mrs, ..
        } = self;
        Ok((
            channels.get_mut(peer).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::AddrNotAvailable,
                    format!("Peer index {} does not exist", peer),
                )
            })?,
            meta_mrs.get_mut(peer).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::AddrNotAvailable,
                    format!("Peer index {} does not exist", peer),
                )
            })?,
        ))
    }
}
