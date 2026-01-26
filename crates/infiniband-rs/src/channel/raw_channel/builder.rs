use crate::channel::raw_channel::RawChannel;
use crate::channel::raw_channel::cached_completion_queue::CachedCompletionQueue;
use crate::ibverbs::prepared_queue_pair::PreparedQueuePair;
use crate::ibverbs::protection_domain::ProtectionDomain;
use crate::ibverbs::queue_pair_builder::AccessFlags;
use crate::ibverbs::queue_pair_endpoint::QueuePairEndpoint;
use bon::bon;
use std::cell::RefCell;
use std::io;
use std::rc::Rc;

#[bon]
impl RawChannel {
    #[builder]
    pub fn builder(
        pd: ProtectionDomain,
        #[builder(default = 32)] min_cq_buf_size: u32,
        #[builder(default = 32)] max_send_wrs: u32,
        #[builder(default = 32)] max_recv_wrs: u32,
        #[builder(default = 32)] max_send_sges: u32,
        #[builder(default = 32)] max_recv_sges: u32,
    ) -> io::Result<PreparedChannel> {
        let cq = pd.context().create_cq(min_cq_buf_size, 0)?;
        let qp = pd
            .create_qp(&cq, &cq)
            .with_access_flags(
                // TODO: Check necessary access after implementing RDMA write read
                AccessFlags::new()
                    .with_local_write()
                    .with_remote_read()
                    .with_remote_write(),
            )
            .with_max_send_wrs(max_send_wrs)
            .with_max_recv_wrs(max_recv_wrs)
            .with_max_send_sges(max_send_sges)
            .with_max_recv_sges(max_recv_sges)
            .build()?;

        Ok(PreparedChannel {
            cq: CachedCompletionQueue::wrap_cq(cq),
            pd,
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

    pub fn handshake(self, endpoint: QueuePairEndpoint) -> io::Result<RawChannel> {
        let qp = self.qp.handshake(endpoint)?;
        Ok(RawChannel {
            qp,
            cq: Rc::new(RefCell::new(self.cq)),
            pd: self.pd,
            next_wr_id: 0,
        })
    }
}
