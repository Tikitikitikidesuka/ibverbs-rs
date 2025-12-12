use std::time::Duration;

use nix::poll::PollTimeout;

use crate::{
    connection::{self, IbConnection, WorkCompletion},
    network::{NetworkNodeError, Result},
};

pub type Rank = usize;

pub struct NetworkNode {
    // vec of connections to all nodes of the network
    // including self
    connections: Vec<IbConnection>,
    rank: Rank,
    poll_timeout: Duration,
}

impl NetworkNode {
    pub fn send(&mut self, peer: Rank, data: &[u8]) -> Result {
        self.get_connection(peer)?
            .send_polled(data)
            .map_err(Into::into)
    }

    pub fn receive(&mut self, peer: Rank, data: &mut [u8]) -> Result {
        self.get_connection(peer)?
            .receive_polled(data)
            .map_err(Into::into)
    }

    pub fn scatter<'a>(&mut self, data: impl Iterator<Item = (Rank, &'a [u8])>) -> Result {
        let requests = data
            .map(|(peer, data)| {
                let connection = self.get_connection(peer)?;
                // SAFETY: we always poll all the work requests to completion before returning.
                unsafe { connection.send_unpolled(data) }.map_err(NetworkNodeError::from)
            })
            .collect::<Vec<_>>();

        // we need to poll all of them to completion, even if an error occurs.
        let results: Vec<_> = requests
            .into_iter()
            .map(|request| {
                request?
                    .spin_poll(self.poll_timeout)
                    .ok_or(NetworkNodeError::PollTimeout)
            })
            .flat_map(Result::err)
            .collect();

        if results.is_empty() {
            Ok(())
        } else {
            Err(NetworkNodeError::MultiOperationError(results))
        }
    }

    pub fn network_size(&self) -> usize {
        self.connections.len()
    }

    fn get_connection(&mut self, peer: usize) -> Result<&mut IbConnection> {
        // todo allow self connection?
        if peer == self.rank {
            return Err(NetworkNodeError::SelfConnection);
        }
        let num_peers = self.connections.len();

        let connection =
            self.connections
                .get_mut(peer)
                .ok_or(NetworkNodeError::PeerOutOfBounds {
                    specified: peer,
                    num_peers,
                })?;

        Ok(connection)
    }
}
