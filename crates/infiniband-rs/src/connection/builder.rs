use crate::connection::cached_completion_queue::CachedCompletionQueue;
use crate::connection::prepared_connection::PreparedConnection;
use crate::context::Context;
use crate::ibverbs::queue_pair_builder::AccessFlags;
use std::io;

#[derive(Debug)]
pub struct ConnectionBuilder<'c> {
    context: &'c Context,
    min_cq_buf_size: u32,
    max_send_wrs: u32,
    max_recv_wrs: u32,
    max_send_sges: u32,
    max_recv_sges: u32,
}

impl<'c> ConnectionBuilder<'c> {
    const DEFAULT_MIN_CQ_BUF_SIZE: u32 = 32;
    const DEFAULT_MAX_WRS: u32 = 32;
    const DEFAULT_MAX_SGES: u32 = 32;

    pub fn new(context: &'c Context) -> Self {
        Self {
            context,
            min_cq_buf_size: Self::DEFAULT_MIN_CQ_BUF_SIZE,
            max_send_wrs: Self::DEFAULT_MAX_WRS,
            max_recv_wrs: Self::DEFAULT_MAX_WRS,
            max_send_sges: Self::DEFAULT_MAX_SGES,
            max_recv_sges: Self::DEFAULT_MAX_SGES,
        }
    }

    pub fn build(&self) -> io::Result<PreparedConnection> {
        let cq = self.context.create_cq(self.min_cq_buf_size, 0)?;
        let pd = self.context.allocate_pd()?;
        let qp = pd
            .create_qp(&cq, &cq)
            .with_access_flags(
                AccessFlags::new()
                    .with_local_write()
                    .with_remote_read()
                    .with_remote_write(),
            )
            .with_max_send_wrs(self.max_send_wrs)
            .with_max_recv_wrs(self.max_recv_wrs)
            .with_max_send_sges(self.max_send_sges)
            .with_max_recv_sges(self.max_send_sges)
            .build()?;

        Ok(PreparedConnection::new(
            CachedCompletionQueue::wrap_cq(cq),
            pd,
            qp,
        ))
    }

    pub fn with_min_cq_buf_size(&mut self, min_cq_buf_size: u32) -> &mut Self {
        self.min_cq_buf_size = min_cq_buf_size;
        self
    }

    pub fn with_max_send_wrs(&mut self, max_send_wrs: u32) -> &mut Self {
        self.max_send_wrs = max_send_wrs;
        self
    }

    pub fn with_max_receive_wrs(&mut self, max_recv_wrs: u32) -> &mut Self {
        self.max_recv_wrs = max_recv_wrs;
        self
    }

    pub fn with_max_elems_per_send_wr(&mut self, max_elems_per_send_wr: u32) -> &mut Self {
        self.max_send_sges = max_elems_per_send_wr;
        self
    }

    pub fn with_max_elems_per_receive_wr(&mut self, max_elems_per_recv_wr: u32) -> &mut Self {
        self.max_recv_sges = max_elems_per_recv_wr;
        self
    }
}
