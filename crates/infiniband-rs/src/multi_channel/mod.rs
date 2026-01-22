mod builder;
mod mr_ops;

use crate::channel::Channel;
use crate::channel::pending_work::{PendingWork};
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use std::io;

pub struct MultiChannel {
    channels: Box<[Channel]>,
    pd: ProtectionDomain,
}

impl MultiChannel {
    pub unsafe fn send_unpolled<'a>(
        &mut self,
        peer: usize,
        sends: impl AsRef<[ScatterElement<'a>]>,
    ) -> io::Result<PendingWork<'a>> {
        unsafe {
            self.channels
                .get_mut(peer)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::AddrNotAvailable,
                        format!("Peer index {} does not exist", peer),
                    )
                })?
                .send_unpolled(sends.as_ref())
        }
    }

    pub unsafe fn send_with_immediate_unpolled<'a>(
        &mut self,
        peer: usize,
        sends: impl AsRef<[ScatterElement<'a>]>,
        imm_data: u32,
    ) -> io::Result<PendingWork<'a>> {
        unsafe {
            self.channels
                .get_mut(peer)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::AddrNotAvailable,
                        format!("Peer index {} does not exist", peer),
                    )
                })?
                .send_with_immediate_unpolled(sends.as_ref(), imm_data)
        }
    }

    pub unsafe fn receive_unpolled<'a>(
        &mut self,
        peer: usize,
        mut receives: impl AsMut<[GatherElement<'a>]>,
    ) -> io::Result<PendingWork<'a>> {
        unsafe {
            self.channels
                .get_mut(peer)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::AddrNotAvailable,
                        format!("Peer index {} does not exist", peer),
                    )
                })?
                .receive_unpolled(receives.as_mut())
        }
    }
}
