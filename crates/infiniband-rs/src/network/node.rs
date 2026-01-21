use crate::connection::builder::ConnectionBuilder;
use crate::connection::connection::Connection;
use crate::connection::work_request::WorkSpinPollResult;
use crate::devices::open_device;
use crate::network::NodeError;
use crate::network::memory_region::NodeMemoryRegion;
use crate::network::network_config::NetworkConfig;
use crate::network::prepared_host::PreparedNode;
use crate::network::scatter_gather_element::NodeScatterElement;
use bon::bon;
use std::io;
use std::marker::PhantomData;

pub type Rank = usize;

pub struct Node {
    connections: Vec<Connection>,
    rank: Rank,
}

impl Node {
    pub(super) fn new(rank: Rank, connections: Vec<Connection>) -> Self {
        Self { rank, connections }
    }
}

#[bon]
impl Node {
    #[builder]
    pub fn builder(rank: Rank, config: &NetworkConfig) -> Result<PreparedNode, NodeError> {
        let self_host = config.get(rank).ok_or(NodeError::RankNotInNetwork {
            rank,
            num_peers: config.len(),
        })?;
        let ctx = open_device(&self_host.ibdev)?;
        let connections = config
            .iter()
            .map(|_host| {
                // todo: allow configuring connection
                ConnectionBuilder::new(&ctx).build()
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(PreparedNode::new(rank, connections))
    }

    pub fn network_size(&self) -> usize {
        self.connections.len()
    }

    pub fn rank(&self) -> Rank {
        self.rank
    }

    /// TODO: on error, some memory regions may be registered
    pub fn register_mr(&mut self, region: &mut [u8]) -> io::Result<NodeMemoryRegion> {
        let connection_mrs = self
            .connections
            .iter_mut()
            .map(|conn| conn.register_mr(region))
            .collect::<Result<_, _>>()?;
        Ok(NodeMemoryRegion::new(connection_mrs))
    }

    /// TODO: on error, some memory regions may be registered
    pub fn register_dmabuf_mr(
        &mut self,
        fd: i32,
        offset: u64,
        length: usize,
        iova: u64,
    ) -> Result<NodeMemoryRegion, ()> {
        todo!()
    }

    pub fn send<'a>(
        &mut self,
        peer: Rank,
        sends: impl AsRef<[NodeScatterElement<'a>]>,
    ) -> WorkSpinPollResult {
        // todo: deal with error of peer not in range
        // todo: avoid allocating somehow?
        let conn_sends: Vec<_> = sends
            .as_ref()
            .iter()
            .map(|se| se.bind(peer))
            .collect::<Result<_, _>>()
            .unwrap();
        self.connections.get_mut(peer).unwrap().send(&conn_sends)
    }

    pub fn send_with_immediate<'a>(
        &mut self,
        peer: Rank,
        sends: impl AsRef<[NodeScatterElement<'a>]>,
        immediate: u32,
    ) -> WorkSpinPollResult {
        // todo: deal with error of peer not in range
        // todo: avoid allocating somehow?
        let conn_sends: Vec<_> = sends
            .as_ref()
            .iter()
            .map(|se| se.bind(peer))
            .collect::<Result<_, _>>()
            .unwrap();
        self.connections
            .get_mut(peer)
            .unwrap()
            .send_with_immediate(&conn_sends, immediate)
    }

    /*
    pub fn receive<'a>(
        &mut self,
        peer: Rank,
        receives: impl AsRef<[NodeGatherElement<'a>]>,
    ) -> WorkSpinPollResult {
        todo!()
    }
    */

    /*
    // network operations

    /// Sends messages to multiple peers in paralell (hardware).
    /// `data`: Iterator over tuples of peer rank, data slice, and optional immediate data.
    // todo also extra here for immediate data?
    pub fn scatter<'a>(
        &mut self,
        data: impl Iterator<Item = (IbvRank, &'a [u8], Option<u32>)>,
    ) -> Result {
        let requests = data
            .map(|(peer, data, immediate)| {
                let connection = self.get_connection(peer)?;
                // SAFETY: we always poll all the work requests to completion before returning.
                unsafe { connection.send_unpolled(data, immediate) }
                    .map_err(IbvNetworkNodeError::from)
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
            Err(IbvNetworkNodeError::MultiOperationError(results))
        }
    }

    /// Receives messages from multiple peers in parallel (hardware).
    ///
    /// Returns the immediate data received from each peer.
    // todo do we want to avoid always having the return vec allocated?
    // todo is there a better way to associate immediate data with the input iterator position it originated from?
    pub fn gather<'a>(
        &mut self,
        data: impl Iterator<Item = (IbvRank, &'a mut [u8])>,
    ) -> Result<Vec<Option<u32>>> {
        let requests = data
            .map(|(peer, data)| {
                let connection = self.get_connection(peer)?;
                // SAFETY: we always poll all the work requests to completion before returning.
                unsafe { connection.receive_unpolled(data) }.map_err(IbvNetworkNodeError::from)
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
            Err(IbvNetworkNodeError::MultiOperationError(results))
        }
    }

    /// Peers shall not include master.
    // todo split into two functions
    pub fn centralized_barrier(
        &mut self,
        master: IbvRank,
        peers: impl Iterator<Item = IbvRank> + Clone,
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
                return Err(IbvNetworkNodeError::BarrierMismatch);
            }

            self.scatter(peers.map(|rank| (rank, &[] as &[u8], Some(barrier_counter))))
        } else {
            self.send_immediate(master, barrier_counter)?;

            let immediate = self.receive_immediate(master)?;

            if immediate != barrier_counter {
                return Err(IbvNetworkNodeError::BarrierMismatch);
            }

            Ok(())
        }
    }

    pub(crate) fn get_connection(&mut self, peer: usize) -> Result<&mut IbvConnection> {
        // todo allow self connection?
        if peer == self.rank {
            return Err(IbvNetworkNodeError::SelfConnection);
        }
        let num_peers = self.connections.len();

        let connection =
            self.connections
                .get_mut(peer)
                .ok_or(IbvNetworkNodeError::PeerOutOfBounds {
                    specified: peer,
                    num_peers,
                })?;

        Ok(connection)
    }

    fn connections_to_other(&mut self) -> impl Iterator<Item = &mut IbvConnection> {
        self.connections
            .iter_mut()
            .enumerate()
            .filter_map(|(i, c)| (i != self.rank).then_some(c))
    }
    */
}
