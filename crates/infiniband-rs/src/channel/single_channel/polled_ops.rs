use crate::channel::raw_channel::pending_work::WorkSpinPollResult;
use crate::channel::single_channel::SingleChannel;
use crate::ibverbs::work_request::{
    ReadWorkRequest, ReceiveWorkRequest, SendWorkRequest, WriteWorkRequest,
};

impl SingleChannel {
    pub fn send<'op>(&'op mut self, wr: SendWorkRequest<'op, 'op>) -> WorkSpinPollResult {
        self.channel.send(wr)
    }

    pub fn receive<'op>(&'op mut self, wr: ReceiveWorkRequest<'op, 'op>) -> WorkSpinPollResult {
        self.channel.receive(wr)
    }

    pub fn write<'op>(&'op mut self, wr: WriteWorkRequest<'op, 'op>) -> WorkSpinPollResult {
        self.channel.write(wr)
    }

    pub fn read<'op>(&'op mut self, wr: ReadWorkRequest<'op, 'op>) -> WorkSpinPollResult {
        self.channel.read(wr)
    }
}
