use crate::channel::Channel;
use crate::multi_channel::MultiChannel;
use std::io;

pub mod polled;
pub mod scoped;
pub mod unpolled;

impl MultiChannel {
    // Helper for single ops
    fn channel(&mut self, peer: usize) -> io::Result<&mut Channel> {
        self.channels.get_mut(peer).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::AddrNotAvailable,
                format!("Peer index {} does not exist", peer),
            )
        })
    }
}
