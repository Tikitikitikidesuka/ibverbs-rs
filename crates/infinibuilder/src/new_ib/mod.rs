mod implementation;
mod unsafe_slice;

use std::ops::RangeBounds;

pub trait SendRecv {
    type Error;

    fn post_send(&mut self, mr_range: impl RangeBounds<usize>) -> Result<impl WorkRequest, Self::Error>;
    fn post_recv(&mut self, mr_range: impl RangeBounds<usize>) -> Result<impl WorkRequest, Self::Error>;
}

pub trait RDMA {
    type Error;

    fn post_write(
        &mut self,
        mr_range: impl RangeBounds<usize>,
        remote_mr_range: impl RangeBounds<usize>,
    ) -> Result<impl WorkRequest, Self::Error>;
    fn post_read(&mut self, mr_range: impl RangeBounds<usize>) -> Result<impl WorkRequest, Self::Error>;
}

pub trait SendRecvImmData {
    type Error;

    fn post_send(&mut self, imm_data: u32) -> Result<impl WorkRequest, Self::Error>;
    fn post_recv(&mut self, imm_data: &mut u32) -> Result<impl WorkRequest, Self::Error>;
}

pub trait SyncBarrier {
    type Error;

    fn barrier(&mut self) -> Result<impl WorkRequest, Self::Error>;
}

pub trait WorkRequest {
    type WorkCompletion;
    type WorkRequestError;

    fn poll(&self) -> Result<WorkRequestStatus<Self::WorkCompletion>, Self::WorkRequestError>;

    fn spin_wait(&self) -> Result<Self::WorkCompletion, Self::WorkRequestError> {
        loop {
            match self.poll()? {
                WorkRequestStatus::Done(wc) => return Ok(wc),
                WorkRequestStatus::Pending => std::hint::spin_loop(),
            }
        }
    }
}

pub enum WorkRequestStatus<WC> {
    Done(WC),
    Pending,
}
