use super::rdma_connection::RdmaConnection;
use std::time::Duration;

// Rank ids in a network must be guaranteed to be
// numeric, strictly monotonic and start at zero in a network
pub type RankId = usize;

trait RdmaNetworkNode {
    type Conn: RdmaConnection;
    type NB: RdmaNetworkBarrier;
    type NT: RdmaNetworkTransport;

    fn barrier(
        &self,
        group: &RdmaNetworkGroup,
        timeout: Duration,
    ) -> Result<(), <Self::NB as RdmaNetworkBarrier>::Error>;

    // TODO: Transport methods
}

pub struct RdmaNetworkGroup {
    rank_ids: Vec<RankId>,
}

pub trait RdmaNetworkGroupConnections {
    fn get(&self, idx: RankId) -> Option<&impl RdmaConnection>;
}

/*
pub struct IbvNetworkGroupConnections<'a, Conn: RdmaConnection> {
    network_connections: &'a [Conn],
    rank_ids: &'a [RankId],
}

impl<'a, Conn: RdmaConnection> RdmaNetworkGroupConnections for IbvNetworkGroupConnections<'a, Conn> {
    fn get(&self, idx: RankId) -> Option<&impl RdmaConnection> {
        let rank_id = self.rank_ids.get(idx)?;
        self.network_connections.get(*rank_id)
    }
}
*/

pub trait RdmaNetworkBarrier {
    type Error;

    fn barrier<Conn: RdmaConnection, GroupConns: RdmaNetworkGroupConnections>(
        &self,
        connections: GroupConns,
        timeout: Duration,
    ) -> Result<(), Self::Error>;
}

pub trait RdmaNetworkTransport {}
