use std::ops::RangeBounds;
use std::time::Duration;
use crate::rdma_traits::WorkRequest;

pub trait NetworkSendRecv {
    unsafe fn post_send(
        &mut self,
        dest_rank_id: usize,
        mr_range: impl RangeBounds<usize>,
        imm_data: Option<u32>,
    ) -> std::io::Result<impl WorkRequest + 'static>;

    unsafe fn post_receive(
        &mut self,
        dest_rank_id: usize,
        mr_range: impl RangeBounds<usize>,
    ) -> std::io::Result<impl WorkRequest + 'static>;
}

pub trait NetworkReadWrite{
    fn post_write(
        &mut self,
        dest_rank_id: usize,
        mr_range: impl RangeBounds<usize>,
        remote_mr_range: impl RangeBounds<usize>,
        imm_data: Option<u32>,
    ) -> std::io::Result<impl WorkRequest + 'static>;

    fn post_read(
        &mut self,
        dest_rank_id: usize,
        mr_range: impl RangeBounds<usize>,
        remote_mr_range: impl RangeBounds<usize>,
    ) -> std::io::Result<impl WorkRequest + 'static>;
}

pub trait NetworkBarrier {
    fn barrier(&mut self) -> std::io::Result<()>;
    fn barrier_timeout(&mut self, timeout: Duration) -> std::io::Result<()>;
}


/*
pub trait NetworkScatter {
}

pub trait NetworkGather {
}
*/



