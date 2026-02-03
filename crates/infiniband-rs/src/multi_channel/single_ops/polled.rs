use crate::channel::TransportResult;
use crate::ibverbs::work_success::WorkSuccess;
use crate::multi_channel::MultiChannel;
use crate::multi_channel::work_request::{
    PeerReadWorkRequest, PeerReceiveWorkRequest, PeerSendWorkRequest, PeerWriteWorkRequest,
};

impl MultiChannel {
    pub fn send<'op>(
        &'op mut self,
        wr: PeerSendWorkRequest<'op, 'op>,
    ) -> TransportResult<WorkSuccess> {
        self.channel(wr.peer)?.send(wr.wr)
    }

    pub fn receive<'op>(
        &'op mut self,
        wr: PeerReceiveWorkRequest<'op, 'op>,
    ) -> TransportResult<WorkSuccess> {
        self.channel(wr.peer)?.receive(wr.wr)
    }

    pub fn write<'op>(
        &'op mut self,
        wr: PeerWriteWorkRequest<'op, 'op>,
    ) -> TransportResult<WorkSuccess> {
        self.channel(wr.peer)?.write(wr.wr)
    }

    pub fn read<'op>(
        &'op mut self,
        wr: PeerReadWorkRequest<'op, 'op>,
    ) -> TransportResult<WorkSuccess> {
        self.channel(wr.peer)?.read(wr.wr)
    }
}
