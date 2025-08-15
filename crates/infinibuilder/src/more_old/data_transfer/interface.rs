use std::ops::RangeBounds;

pub trait IbBNodeDataTransfer {
    type DataTransferRequest;

    fn post_send(
        &self,
        rank_id: u32,
        memory_range: impl RangeBounds<usize>,
    ) -> std::io::Result<Self::DataTransferRequest>;
    fn post_receive(
        &self,
        rank_id: u32,
        memory_range: impl RangeBounds<usize>,
    ) -> std::io::Result<Self::DataTransferRequest>;
}

pub enum IbBWorkRequestStatus<T> {
    Pending,
    Done(T),
}

pub trait IbBDataTransferRequest {
    type DataTransferCompletion;

    fn poll(&self) -> std::io::Result<IbBWorkRequestStatus<Self::DataTransferCompletion>>;

    fn wait_barrier(self) -> std::io::Result<Self::DataTransferCompletion>
    where
        Self: Sized,
    {
        use IbBWorkRequestStatus::*;

        loop {
            match self.poll()? {
                Pending => std::hint::spin_loop(),
                Done(wc) => return Ok(wc),
            }
        }
    }
}
