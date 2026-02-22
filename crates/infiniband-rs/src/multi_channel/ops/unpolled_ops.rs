use crate::channel::pending_work::PendingWork;
use crate::ibverbs::error::IbvResult;
use crate::ibverbs::work::SendWorkRequest;
use crate::multi_channel::MultiChannel;
use crate::multi_channel::work_request::*;

impl MultiChannel {
    pub unsafe fn scatter_send_unpolled<'wr, 'data, I>(
        &mut self,
        wrs: I,
    ) -> IbvResult<Vec<PendingWork<'data>>>
    where
        I: IntoIterator<Item = PeerSendWorkRequest<'wr, 'data>>,
        'data: 'wr,
    {
        wrs.into_iter()
            .map(|wr| unsafe { self.send_unpolled(wr) })
            .collect()
    }

    pub unsafe fn scatter_write_unpolled<'wr, 'data, I>(
        &mut self,
        wrs: I,
    ) -> IbvResult<Vec<PendingWork<'data>>>
    where
        I: IntoIterator<Item = PeerWriteWorkRequest<'wr, 'data>>,
        'data: 'wr,
    {
        wrs.into_iter()
            .map(|wr| unsafe { self.write_unpolled(wr) })
            .collect()
    }

    pub unsafe fn gather_receive_unpolled<'wr, 'data, I>(
        &mut self,
        wrs: I,
    ) -> IbvResult<Vec<PendingWork<'data>>>
    where
        I: IntoIterator<Item = PeerReceiveWorkRequest<'wr, 'data>>,
        'data: 'wr,
    {
        wrs.into_iter()
            .map(|wr| unsafe { self.receive_unpolled(wr) })
            .collect()
    }

    pub unsafe fn gather_read_unpolled<'wr, 'data, I>(
        &mut self,
        wrs: I,
    ) -> IbvResult<Vec<PendingWork<'data>>>
    where
        I: IntoIterator<Item = PeerReadWorkRequest<'wr, 'data>>,
        'data: 'wr,
    {
        wrs.into_iter()
            .map(|wr| unsafe { self.read_unpolled(wr) })
            .collect()
    }

    pub unsafe fn multicast_send_unpolled<'wr, 'data, I>(
        &mut self,
        peers: I,
        wr: SendWorkRequest<'wr, 'data>,
    ) -> IbvResult<Vec<PendingWork<'data>>>
    where
        I: IntoIterator<Item = usize>,
    {
        peers
            .into_iter()
            .map(|peer| unsafe {
                self.send_unpolled(PeerSendWorkRequest::from_wr(peer, wr.clone()))
            })
            .collect()
    }
}
