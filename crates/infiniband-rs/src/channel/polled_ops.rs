use crate::channel::Channel;
use crate::channel::pending_work::{WorkPollError, WorkSpinPollResult};
use crate::channel::polling_scope::ScopeError;
use crate::ibverbs::work_request::{
    ReadWorkRequest, ReceiveWorkRequest, SendWorkRequest, WriteWorkRequest,
};
use crate::ibverbs::work_success::WorkSuccess;

impl Channel {
    pub fn send<'op>(&'op mut self, wr: SendWorkRequest<'op, 'op>) -> WorkSpinPollResult {
        self.manual_scope(|s| s.post_send(wr)?.spin_poll())
    }

    pub fn receive<'op>(&'op mut self, wr: ReceiveWorkRequest<'op, 'op>) -> WorkSpinPollResult {
        self.manual_scope(|s| s.post_receive(wr)?.spin_poll())
    }

    pub fn write<'op>(&'op mut self, wr: WriteWorkRequest<'op, 'op>) -> WorkSpinPollResult {
        self.manual_scope(|s| s.post_write(wr)?.spin_poll())
    }

    pub fn read<'op>(&'op mut self, wr: ReadWorkRequest<'op, 'op>) -> WorkSpinPollResult {
        self.manual_scope(|s| s.post_read(wr)?.spin_poll())
    }
}
