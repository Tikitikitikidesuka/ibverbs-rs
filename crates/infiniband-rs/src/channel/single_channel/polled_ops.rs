use crate::channel::raw_channel::pending_work::WorkSpinPollResult;
use crate::channel::single_channel::SingleChannel;
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use crate::ibverbs::work_request::{ReceiveWorkRequest, SendWorkRequest};

impl SingleChannel {
    pub fn send<'a, E: AsRef<[GatherElement<'a>]>>(
        &'a mut self,
        wr: SendWorkRequest<'a, E>,
    ) -> WorkSpinPollResult {
        self.channel.send(wr)
    }

    pub fn receive<'a, E: AsMut<[ScatterElement<'a>]>>(
        &'a mut self,
        wr: ReceiveWorkRequest<'a, E>,
    ) -> WorkSpinPollResult {
        self.channel.receive(wr)
    }
}
