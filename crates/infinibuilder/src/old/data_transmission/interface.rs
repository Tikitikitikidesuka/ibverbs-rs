use std::ops::RangeBounds;

pub struct Connection;

pub trait IbBDataTransmissionConnection {
    fn post_send(connection: Connection, )
}

/*
pub trait IbBDataTransmissionConnection {
    type WorkRequest: IbBDataTransmissionWorkRequest<Self::WorkCompletion>;
    type WorkCompletion;

    fn post_send(&mut self, bounds: impl RangeBounds<usize>) -> std::io::Result<Self::WorkRequest>;
    fn post_receive(
        &mut self,
        bounds: impl RangeBounds<usize>,
    ) -> std::io::Result<Self::WorkRequest>;
}

pub trait IbBDataTransmissionWorkRequest<WC> {
    fn poll(&self) -> std::io::Result<WorkRequestStatus<WC>>;
}

pub enum WorkRequestStatus<WC> {
    Done(WC),
    Waiting,
}
*/