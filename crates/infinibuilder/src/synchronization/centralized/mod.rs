use crate::network::NetworkOp;
use crate::rdma_traits::{RdmaRendezvous, RdmaSendRecv};

pub struct CentralizedSync;

impl NetworkOp for CentralizedSync {
    type Output = std::io::Result<()>;

    fn run<'a, C: Iterator<Item = &'a mut T>, T: 'a + RdmaSendRecv + RdmaRendezvous>(
        &self,
        mut connections: C,
    ) -> Self::Output {
        connections.try_for_each(|connection| connection.rendezvous())
    }
}
