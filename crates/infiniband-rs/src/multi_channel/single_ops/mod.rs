use crate::channel::Channel;
use crate::ibverbs::error::{IbvError, IbvResult};
use crate::multi_channel::MultiChannel;
use std::io;

pub mod polled;
pub mod scoped;
pub mod unpolled;

impl MultiChannel {
    pub(crate) fn channel(&mut self, peer: usize) -> IbvResult<&mut Channel> {
        self.channels
            .get_mut(peer)
            .ok_or_else(|| IbvError::NotFound(format!("Peer {peer} not found")))
    }
}
