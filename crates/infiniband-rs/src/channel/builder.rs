use crate::channel::Channel;
use crate::channel::cached_completion_queue::CachedCompletionQueue;
use crate::ibverbs::access_config::AccessFlags;
use crate::ibverbs::error::IbvResult;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::ibverbs::queue_pair::builder::{PreparedQueuePair, QueuePairEndpoint};
use crate::ibverbs::queue_pair::config::*;
use bon::bon;
use std::cell::RefCell;
use std::rc::Rc;

#[bon]
impl Channel {
    #[builder(state_mod(vis = "pub(crate)"))]
    pub fn builder(
        pd: &ProtectionDomain,
        #[builder(default =
            AccessFlags::new()
                .with_local_write()
                .with_remote_read()
                .with_remote_write()
        )]
        access: AccessFlags,
        #[builder(default = 32)] min_cq_entries: u32,
        #[builder(default = 16)] max_send_wr: u32,
        #[builder(default = 16)] max_recv_wr: u32,
        #[builder(default = 16)] max_send_sge: u32,
        #[builder(default = 16)] max_recv_sge: u32,
        #[builder(default)] max_rnr_retries: MaxRnrRetries,
        #[builder(default)] max_ack_retries: MaxAckRetries,
        #[builder(default)] min_rnr_timer: MinRnrTimer,
        #[builder(default)] ack_timeout: AckTimeout,
        #[builder(default)] mtu: MaximumTransferUnit,
        #[builder(default)] send_psn: PacketSequenceNumber,
        #[builder(default)] recv_psn: PacketSequenceNumber,
    ) -> IbvResult<PreparedChannel> {
        let cq = pd.context().create_cq(0, min_cq_entries)?;
        let qp = pd
            .create_qp()
            .send_cq(&cq)
            .recv_cq(&cq)
            .access(access)
            .max_send_wr(max_send_wr)
            .max_recv_wr(max_recv_wr)
            .max_send_sge(max_send_sge)
            .max_recv_sge(max_recv_sge)
            .max_rnr_retries(max_rnr_retries)
            .max_ack_retries(max_ack_retries)
            .min_rnr_timer(min_rnr_timer)
            .ack_timeout(ack_timeout)
            .mtu(mtu)
            .send_psn(send_psn)
            .recv_psn(recv_psn)
            .build()?;

        Ok(PreparedChannel {
            cq: CachedCompletionQueue::wrap_cq(cq),
            pd: pd.clone(),
            qp,
        })
    }
}

pub struct PreparedChannel {
    cq: CachedCompletionQueue,
    pd: ProtectionDomain,
    qp: PreparedQueuePair,
}

impl PreparedChannel {
    pub fn endpoint(&self) -> QueuePairEndpoint {
        self.qp.endpoint()
    }

    pub fn handshake(self, endpoint: QueuePairEndpoint) -> IbvResult<Channel> {
        let qp = self.qp.handshake(endpoint)?;
        Ok(Channel {
            qp,
            cq: Rc::new(RefCell::new(self.cq)),
            pd: self.pd,
            next_wr_id: 0,
        })
    }
}
