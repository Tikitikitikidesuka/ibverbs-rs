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
        self.get_connection(peer)?.send(data).map_err(Into::into)
    }

    pub fn send_immediate(&mut self, peer: Rank, immediate: u32) -> Result {
        self.get_connection(peer)?
            .send_immediate(immediate)
            .map_err(Into::into)
    }

    pub fn send_with_immediate(&mut self, peer: Rank, data: &[u8], immediate: u32) -> Result {
        self.get_connection(peer)?
            .send_with_immediate(data, immediate)
            .map_err(Into::into)
    }

    pub fn receive(&mut self, peer: Rank, data: &mut [u8]) -> Result<Option<u32>> {
        self.get_connection(peer)?.receive(data).map_err(Into::into)
    }

    /// Sends messages to multiple peers in paralell (hardware).
    /// `data`: Iterator over tuples of peer rank, data slice, and optional immediate data.
    // todo also extra here for immediate data?
    pub fn scatter<'a>(
        &mut self,
        data: impl Iterator<Item = (Rank, &'a [u8], Option<u32>)>,
    ) -> Result {
        let requests = data
            .map(|(peer, data, immediate)| {
                let connection = self.get_connection(peer)?;
                // SAFETY: we always poll all the work requests to completion before returning.
                unsafe { connection.send_unpolled(data, immediate) }.map_err(NetworkNodeError::from)
            })
            .collect::<Vec<_>>();

        // we need to poll all of them to completion, even if an error occurs.
        let results: Vec<_> = requests
            .into_iter()
            .map(|request| {
                Ok(request?
                    .spin_poll(self.poll_timeout)
                    .expect("poll timed out")) // we cannot return an error here, as the slice is still used.
            })
            .flat_map(Result::err)
            .collect();

        if results.is_empty() {
            Ok(())
        } else {
            Err(NetworkNodeError::MultiOperationError(results))
        }
    }

    /// Receives messages from multiple peers in parallel (hardware).
    ///
    /// Returns the immediate data received from each peer.
    // todo do we want to avoid always having the return vec allocated?
    // todo is there a better way to associate immediate data with the input iterator position it originated from?
    pub fn gather<'a>(
        &mut self,
        data: impl Iterator<Item = (Rank, &'a mut [u8])>,
    ) -> Result<Vec<Option<u32>>> {
        let requests = data
            .map(|(peer, data)| {
                let connection = self.get_connection(peer)?;
                // SAFETY: we always poll all the work requests to completion before returning.
                unsafe { connection.receive_unpolled(data) }.map_err(NetworkNodeError::from)
            })
            .collect::<Vec<_>>();

        // we need to poll all of them to completion, even if an error occurs.
        let mut immediates = Vec::with_capacity(requests.len());

        let results: Vec<_> = requests
            .into_iter()
            .map(|request| {
                let completion = request?
                    .spin_poll(self.poll_timeout)
                    .expect("poll timed out")?; // we cannot return an error here, as the slice is still used.
                immediates.push(completion.immediate_data());
                Ok(())
            })
            .flat_map(Result::err)
            .collect();

        if results.is_empty() {
            Ok(immediates)
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
