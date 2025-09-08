use ibverbs::ibv_wc;

pub enum TransferRequestStatus<T> {
    Pending,
    Done(T),
}

pub struct TransferRequest {}

impl TransferRequest {
    pub fn poll(&self) -> std::io::Result<TransferRequestStatus<ibv_wc>> {
        todo!()
    }

    pub fn wait(self) -> std::io::Result<ibv_wc>
    where
        Self: Sized,
    {
        use TransferRequestStatus::*;

        loop {
            match self.poll()? {
                Pending => std::hint::spin_loop(),
                Done(wc) => return Ok(wc),
            }
        }
    }
}