use crate::channel::{Channel, TransportResult};
use crate::ibverbs::work::{
    ReadWorkRequest, ReceiveWorkRequest, SendWorkRequest, WorkSuccess, WriteWorkRequest,
};

impl Channel {
    /// Posts a send operation and blocks until it completes.
    pub fn send<'op>(&'op mut self, wr: SendWorkRequest<'op, 'op>) -> TransportResult<WorkSuccess> {
        self.manual_scope(|s| s.post_send(wr)?.spin_poll())
    }

    /// Posts a receive operation and blocks until it completes.
    pub fn receive<'op>(
        &'op mut self,
        wr: ReceiveWorkRequest<'op, 'op>,
    ) -> TransportResult<WorkSuccess> {
        self.manual_scope(|s| s.post_receive(wr)?.spin_poll())
    }

    /// Posts an RDMA write operation and blocks until it completes.
    pub fn write<'op>(
        &'op mut self,
        wr: WriteWorkRequest<'op, 'op>,
    ) -> TransportResult<WorkSuccess> {
        self.manual_scope(|s| s.post_write(wr)?.spin_poll())
    }

    /// Posts an RDMA read operation and blocks until it completes.
    pub fn read<'op>(&'op mut self, wr: ReadWorkRequest<'op, 'op>) -> TransportResult<WorkSuccess> {
        self.manual_scope(|s| s.post_read(wr)?.spin_poll())
    }
}
