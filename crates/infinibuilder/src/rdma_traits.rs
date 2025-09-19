use std::ops::RangeBounds;
use std::time::Duration;

pub trait RdmaSendRecv {
    unsafe fn post_send(
        &mut self,
        mr_range: impl RangeBounds<usize>,
        imm_data: Option<u32>,
    ) -> std::io::Result<impl WorkRequest>;

    unsafe fn post_receive(
        &mut self,
        mr_range: impl RangeBounds<usize>,
    ) -> std::io::Result<impl WorkRequest>;
}

pub trait SafeRdmaSendRecv {
    fn post_send(
        &mut self,
        mr_range: impl RangeBounds<usize>,
        imm_data: Option<u32>,
    ) -> std::io::Result<impl WorkRequest>;

    fn post_receive(
        &mut self,
        mr_range: impl RangeBounds<usize>,
    ) -> std::io::Result<impl WorkRequest>;
}

pub trait RdmaReadWrite {
    unsafe fn post_write(
        &mut self,
        mr_range: impl RangeBounds<usize>,
        remote_mr_range: impl RangeBounds<usize>,
        imm_data: Option<u32>,
    ) -> std::io::Result<impl WorkRequest>;

    unsafe fn post_read(
        &mut self,
        mr_range: impl RangeBounds<usize>,
        remote_mr_slice: impl RangeBounds<usize>,
    ) -> std::io::Result<impl WorkRequest>;
}

pub trait SafeRdmaReadWrite {
    fn post_write(
        &mut self,
        mr_range: impl RangeBounds<usize>,
        remote_mr_range: impl RangeBounds<usize>,
        imm_data: Option<u32>,
    ) -> std::io::Result<impl WorkRequest>;

    fn post_read(
        &mut self,
        mr_range: impl RangeBounds<usize>,
        remote_mr_slice: impl RangeBounds<usize>,
    ) -> std::io::Result<impl WorkRequest>;
}

pub trait RdmaRendezvous {
    fn rendezvous(&mut self) -> std::io::Result<()>;
    fn rendezvous_timeout(&mut self, timeout: Duration) -> std::io::Result<()>;
}

pub trait WorkRequest {
    fn poll(&mut self) -> std::io::Result<Option<WorkCompletion>>;
    fn wait(self) -> std::io::Result<WorkCompletion>;
    fn wait_timeout(self, timeout: Duration) -> std::io::Result<WorkCompletion>;
}

#[derive(Debug, Copy, Clone)]
pub struct WorkCompletion {
    pub len: usize,
    pub imm_data: Option<u32>,
}
