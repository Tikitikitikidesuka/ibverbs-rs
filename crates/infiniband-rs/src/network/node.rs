use std::time::Duration;

use nix::poll::PollTimeout;

use crate::{
    connection::{self, IbConnection, RemoteMrSlice, WorkCompletion, WorkRequest},
    network::{NetworkNodeError, Result},
};

pub type Rank = usize;

pub struct NetworkNode {
    // vec of connections to all nodes of the network
    // including self
    pub(crate) connections: Vec<IbConnection>,
    rank: Rank,
    poll_timeout: Duration,
    barrier_counter: u32,
}

impl NetworkNode {
    pub fn network_size(&self) -> usize {
        self.connections.len()
    }

    pub fn rank(&self) -> Rank {
        self.rank
    }

    // memory regions

    /// on error, some memory regions may be registered
    pub fn register_mr(&mut self, name: impl Into<String>, region: *mut [u8]) -> Result {
        let name = name.into();
        self.connections_to_other()
            .map(|conn| {
                conn.register_mr(name.clone(), region)
                    .map_err(NetworkNodeError::from)
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(())
    }

    /// on error, some memory regions may be registered
    pub fn register_dmabuf_mr(
        &mut self,
        name: impl Into<String>,
        fd: i32,
        region: *mut [u8],
    ) -> Result<()> {
        let name = name.into();
        self.connections_to_other()
            .map(|conn| {
                conn.register_dmabuf_mr(name.clone(), fd, region)
                    .map_err(NetworkNodeError::from)
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(())
    }

    // todo not clear how to share memory regions

    // // Safety: When sharing an mr, it is exposed to be mutated remotely
    // // by the peer at any point. It is the user's responsibility to ensure
    // // a protocol to comply with Rust's memory safety guarantees.
    // pub unsafe fn share_mr(&mut self, name: impl AsRef<str>) -> Result {
    //     self.connections_to_other()
    //         .map(|conn| unsafe { conn.share_mr(&name) }.map_err(NetworkNodeError::from))
    //         .collect::<Result<Vec<_>>>()?;

    //     Ok(())
    // }

    // pub fn accept_shared_mr(&mut self) -> Result<RemoteMr> {
    //     self.connections_to_other().map(|conn| conn.acce)
    // }

    // pub fn remote_mr(&mut self, name: impl AsRef<str>) -> Option<RemoteMr> {
    //     //self.inner.remote_mr(name)
    //     todo!()
    // }

    /// on error, some memory regions may still be registered
    pub fn deregister_mr(&mut self, name: impl AsRef<str>) -> Result {
        self.connections_to_other()
            .map(|conn| conn.deregister_mr(&name).map_err(NetworkNodeError::from))
            .collect::<Result<Vec<_>>>()?;

        Ok(())
    }

    // send / receive

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

    pub fn receive_immediate(&mut self, peer: Rank) -> Result<u32> {
        self.get_connection(peer)?
            .receive_immediate()
            .map_err(Into::into)
    }

    /// # Safety
    /// This method is unsafe because ...
    /// todo, do we need to make it unsafe if it does unsafe things on the *other* side?
    ///
    /// Furthermore, he caller must ensure that the work request is sucessfully polled to completion before the end of `'a`.
    pub unsafe fn remote_write<'a>(
        &mut self,
        peer: Rank,
        data: &'a [u8],
        remote_slice: RemoteMrSlice,
    ) -> Result<WorkRequest<'a>> {
        let connection = self.get_connection(peer)?;
        unsafe { connection.remote_write(data, remote_slice) }.map_err(Into::into)
    }

    /// # Safety
    /// This method is unsafe because ...
    /// todo
    ///
    /// Furthermore, the caller must ensure that the work request is sucessfully polled to completion before the end of `'a`.
    pub unsafe fn remote_read<'a>(
        &mut self,
        peer: Rank,
        remote_slice: RemoteMrSlice,
        data: &'a mut [u8],
    ) -> Result<WorkRequest<'a>> {
        let connection = self.get_connection(peer)?;
        unsafe { connection.remote_read(remote_slice, data) }.map_err(Into::into)
    }

    // network operations

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

    /// Peers shall not include master.
    // todo split into two functions
    pub fn centralized_barrier(
        &mut self,
        master: Rank,
        peers: impl Iterator<Item = Rank> + Clone,
        timeout: Duration,
    ) -> Result {
        let barrier_counter = self.barrier_counter;
        self.barrier_counter += 1;

        // todo timeout?
        if self.rank == master {
            if !self
                .gather(peers.clone().map(|r| (r, &mut [] as &mut [u8])))?
                .iter()
                .all(|b| *b == Some(barrier_counter))
            {
                return Err(NetworkNodeError::BarrierMismatch);
            }

            self.scatter(peers.map(|rank| (rank, &[] as &[u8], Some(barrier_counter))))
        } else {
            self.send_immediate(master, barrier_counter)?;

            let immediate = self.receive_immediate(master)?;

            if immediate != barrier_counter {
                return Err(NetworkNodeError::BarrierMismatch);
            }

            Ok(())
        }
    }

    pub(crate) fn get_connection(&mut self, peer: usize) -> Result<&mut IbConnection> {
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

    fn connections_to_other(&mut self) -> impl Iterator<Item = &mut IbConnection> {
        self.connections
            .iter_mut()
            .enumerate()
            .filter_map(|(i, c)| (i != self.rank).then_some(c))
    }
}
